use super::closure::Closure;
use crate::{Array, Function, Integer, Real, Reference, Transferable};
use intuicio_core::{
    IntuicioStruct, context::Context, function::FunctionQuery, host::HostProducer,
    registry::Registry,
};
use intuicio_derive::{IntuicioStruct, intuicio_method, intuicio_methods};
use std::{
    collections::VecDeque,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{JoinHandle, available_parallelism, spawn},
    time::Duration,
};

type WorkerQueue = Arc<RwLock<VecDeque<JobRequest>>>;
type JobResult = Arc<RwLock<JobState>>;

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Jobs", module_name = "jobs", override_send = false)]
pub struct Jobs {
    #[intuicio(ignore)]
    workers: Vec<Worker>,
}

#[intuicio_methods(module_name = "jobs")]
impl Jobs {
    pub const HOST_PRODUCER_CUSTOM: &'static str = "Jobs::host_producer";

    #[allow(clippy::new_ret_no_self)]
    #[intuicio_method(use_context, use_registry)]
    pub fn new(context: &Context, registry: &Registry, workers_count: Reference) -> Reference {
        let host_producer = match context.custom::<HostProducer>(Self::HOST_PRODUCER_CUSTOM) {
            Some(host_producer) => host_producer.clone(),
            None => return Reference::null(),
        };
        let workers_count = workers_count
            .read::<Integer>()
            .map(|count| *count as usize)
            .unwrap_or_else(|| {
                available_parallelism()
                    .map(|count| count.get())
                    .unwrap_or_default()
            });
        Reference::new(
            Self {
                workers: (0..workers_count)
                    .map(|_| Worker::new(host_producer.clone()))
                    .collect(),
            },
            registry,
        )
    }

    #[intuicio_method()]
    pub fn sleep(seconds: Reference) -> Reference {
        std::thread::sleep(Duration::from_secs_f64(*seconds.read::<Real>().unwrap()));
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn workers(registry: &Registry, jobs: Reference) -> Reference {
        let jobs = jobs.read::<Jobs>().unwrap();
        Reference::new_integer(jobs.workers.len() as Integer, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn workers_alive(registry: &Registry, jobs: Reference) -> Reference {
        let jobs = jobs.read::<Jobs>().unwrap();
        Reference::new_array(
            jobs.workers
                .iter()
                .map(|worker| {
                    Reference::new_boolean(worker.is_running.load(Ordering::SeqCst), registry)
                })
                .collect(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn schedule(
        registry: &Registry,
        jobs: Reference,
        executor: Reference,
        arguments: Reference,
    ) -> Reference {
        let jobs = jobs.read::<Jobs>().unwrap();
        let arguments = arguments.read::<Array>().unwrap();
        let (function_name, function_module_name, captured) =
            if let Some(function) = executor.read::<Function>() {
                let signature = function.handle().unwrap().signature();
                (
                    signature.name.to_owned(),
                    signature.module_name.to_owned(),
                    vec![],
                )
            } else if let Some(closure) = executor.read::<Closure>() {
                let signature = closure.function.handle().unwrap().signature();
                (
                    signature.name.to_owned(),
                    signature.module_name.to_owned(),
                    closure.captured.to_owned(),
                )
            } else {
                return Reference::null();
            };
        let worker = jobs
            .workers
            .iter()
            .filter(|worker| {
                worker
                    .handle
                    .as_ref()
                    .map(|handle| !handle.is_finished())
                    .unwrap_or_default()
            })
            .min_by(|a, b| {
                let a = a
                    .queue
                    .try_read()
                    .map(|queue| queue.len())
                    .unwrap_or_default();
                let b = b
                    .queue
                    .try_read()
                    .map(|queue| queue.len())
                    .unwrap_or_default();
                a.cmp(&b)
            });
        if let Some(worker) = worker {
            return Reference::new(
                worker.schedule(function_name, function_module_name, &captured, &arguments),
                registry,
            );
        }
        Reference::null()
    }
}

struct Worker {
    handle: Option<JoinHandle<()>>,
    is_running: Arc<AtomicBool>,
    queue: WorkerQueue,
    _running_job_result: Arc<RwLock<Option<JobResult>>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Worker {
    pub fn new(host_producer: HostProducer) -> Self {
        let queue = WorkerQueue::default();
        let queue_ = queue.clone();
        let is_running = Arc::new(AtomicBool::new(false));
        let is_running_ = is_running.clone();
        let _running_job_result: Arc<RwLock<Option<JobResult>>> = Default::default();
        let running_job_result = _running_job_result.clone();
        Self {
            handle: Some(spawn(move || {
                Self::worker_thread(host_producer, is_running_, queue_, running_job_result);
            })),
            is_running,
            queue,
            _running_job_result,
        }
    }

    pub fn schedule(
        &self,
        function_name: String,
        function_module_name: Option<String>,
        captured: &[Reference],
        arguments: &[Reference],
    ) -> Job {
        let result = Job::default();
        if let Ok(mut queue) = self.queue.write() {
            queue.push_back(JobRequest {
                function_name,
                function_module_name,
                arguments: captured
                    .iter()
                    .chain(arguments.iter())
                    .map(|argument| Transferable::from(argument.clone()))
                    .collect(),
                result: result.result.clone(),
            });
        }
        result
    }

    fn consume_requests(
        is_running: &Arc<AtomicBool>,
        queue: &Arc<RwLock<VecDeque<JobRequest>>>,
        running_job_result: &Arc<RwLock<Option<JobResult>>>,
    ) {
        is_running.store(false, Ordering::SeqCst);
        if let Ok(mut result) = running_job_result.write() {
            if let Some(result) = result.as_mut() {
                if let Ok(mut result) = result.write() {
                    *result = JobState::Consumed;
                }
            }
        }
        if let Ok(mut queue) = queue.write() {
            while let Some(request) = queue.pop_front() {
                if let Ok(mut result) = request.result.write() {
                    *result = JobState::Consumed;
                }
            }
        }
    }

    fn worker_thread(
        host_producer: HostProducer,
        is_running: Arc<AtomicBool>,
        queue: Arc<RwLock<VecDeque<JobRequest>>>,
        running_job_result: Arc<RwLock<Option<JobResult>>>,
    ) {
        let panic_hook = std::panic::take_hook();
        let is_running_ = is_running.clone();
        let queue_ = queue.clone();
        let running_job_result_ = running_job_result.clone();
        std::panic::set_hook(Box::new(move |info| {
            Self::consume_requests(&is_running_, &queue_, &running_job_result_);
            panic_hook(info);
        }));

        let mut host = host_producer.produce();
        host.context()
            .set_custom(Jobs::HOST_PRODUCER_CUSTOM, host_producer);
        is_running.store(true, Ordering::SeqCst);
        while is_running.load(Ordering::SeqCst) {
            let request = queue
                .try_write()
                .ok()
                .and_then(|mut queue| queue.pop_front());
            if let Some(request) = request {
                let (context, registry) = host.context_and_registry();
                if let Some(function) = registry.find_function(FunctionQuery {
                    name: Some(request.function_name.into()),
                    module_name: request.function_module_name.map(|name| name.into()),
                    ..Default::default()
                }) {
                    if let Ok(mut result) = running_job_result.write() {
                        *result = Some(request.result.clone());
                    }
                    if let Ok(mut result) = request.result.write() {
                        *result = JobState::Running;
                    }
                    for argument in request.arguments.into_iter().rev() {
                        context.stack().push(Reference::from(argument));
                    }
                    function.invoke(context, registry);
                    let output = Transferable::from(context.stack().pop::<Reference>().unwrap());
                    if let Ok(mut result) = request.result.write() {
                        *result = JobState::Done(output);
                    }
                    if let Ok(mut result) = running_job_result.write() {
                        *result = None;
                    }
                }
            }
        }
        is_running.store(false, Ordering::SeqCst);
        Self::consume_requests(&is_running, &queue, &running_job_result);
    }
}

struct JobRequest {
    function_name: String,
    function_module_name: Option<String>,
    arguments: Vec<Transferable>,
    result: JobResult,
}

#[derive(Default)]
enum JobState {
    #[default]
    Pending,
    Running,
    Done(Transferable),
    Consumed,
}

impl JobState {
    fn consume(&mut self) -> Reference {
        let state = std::mem::replace(self, JobState::Consumed);
        if let Self::Done(transferable) = state {
            Reference::from(transferable)
        } else {
            *self = state;
            Reference::null()
        }
    }
}

#[derive(IntuicioStruct, Default, Clone)]
#[intuicio(name = "Job", module_name = "job", override_send = false)]
pub struct Job {
    #[intuicio(ignore)]
    result: JobResult,
}

#[intuicio_methods(module_name = "job")]
impl Job {
    #[intuicio_method(use_registry)]
    pub fn is_pending(registry: &Registry, job: Reference) -> Reference {
        let job = job.read::<Job>().unwrap();
        Reference::new_boolean(
            job.result
                .try_read()
                .map(|state| matches!(*state, JobState::Pending))
                .unwrap_or_default(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn is_running(registry: &Registry, job: Reference) -> Reference {
        let job = job.read::<Job>().unwrap();
        Reference::new_boolean(
            job.result
                .try_read()
                .map(|state| matches!(*state, JobState::Running))
                .unwrap_or_default(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn is_done(registry: &Registry, job: Reference) -> Reference {
        let job = job.read::<Job>().unwrap();
        Reference::new_boolean(
            job.result
                .try_read()
                .map(|state| matches!(*state, JobState::Done(_)))
                .unwrap_or_default(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn is_consumed(registry: &Registry, job: Reference) -> Reference {
        let job = job.read::<Job>().unwrap();
        Reference::new_boolean(
            job.result
                .try_read()
                .map(|state| matches!(*state, JobState::Consumed))
                .unwrap_or_default(),
            registry,
        )
    }

    #[intuicio_method()]
    pub fn consume(mut job: Reference) -> Reference {
        let job = job.write::<Job>().unwrap();
        if let Ok(mut state) = job.result.try_write() {
            return state.consume();
        }
        Reference::null()
    }

    #[intuicio_method()]
    pub fn wait_then_consume(mut job: Reference) -> Reference {
        let job = job.write::<Job>().unwrap();
        loop {
            if let Ok(mut state) = job.result.try_write() {
                if matches!(*state, JobState::Done(_)) {
                    return state.consume();
                } else if matches!(*state, JobState::Consumed) {
                    return Reference::null();
                }
            }
        }
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_type(Jobs::define_struct(registry));
    registry.add_type(Job::define_struct(registry));
    registry.add_function(Jobs::new__define_function(registry));
    registry.add_function(Jobs::sleep__define_function(registry));
    registry.add_function(Jobs::workers__define_function(registry));
    registry.add_function(Jobs::workers_alive__define_function(registry));
    registry.add_function(Jobs::schedule__define_function(registry));
    registry.add_function(Job::is_pending__define_function(registry));
    registry.add_function(Job::is_running__define_function(registry));
    registry.add_function(Job::is_done__define_function(registry));
    registry.add_function(Job::is_consumed__define_function(registry));
    registry.add_function(Job::consume__define_function(registry));
    registry.add_function(Job::wait_then_consume__define_function(registry));
}
