use intuicio_core::{
    IntuicioStruct, IntuicioVersion, context::Context, core_version, function::Function,
    registry::Registry,
};
use intuicio_derive::{IntuicioStruct, intuicio_method, intuicio_methods};
use intuicio_frontend_simpleton::{Boolean, Integer, Real, Reference, library::closure::Closure};
use rstar::{AABB, Envelope, Point, PointDistance, RTree, RTreeObject, primitives::GeomWithData};

#[derive(Clone)]
pub struct Sphere {
    x: Real,
    y: Real,
    z: Real,
    radius: Real,
    user: Reference,
}

impl PartialEq for Sphere {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x
            && self.y == other.y
            && self.z == other.z
            && self.user.does_share_reference(&other.user, true)
    }
}

impl RTreeObject for Sphere {
    type Envelope = AABB<[Real; 3]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(
            [
                self.x - self.radius,
                self.y - self.radius,
                self.z - self.radius,
            ],
            [
                self.x + self.radius,
                self.y + self.radius,
                self.z + self.radius,
            ],
        )
    }
}

impl PointDistance for Sphere {
    fn distance_2(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
    ) -> <<Self::Envelope as Envelope>::Point as Point>::Scalar {
        let dx = self.x - point[0];
        let dy = self.y - point[1];
        let dz = self.z - point[2];
        dx * dx + dy * dy + dz * dz - self.radius * self.radius
    }
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "RtreeResult", module_name = "rtree")]
pub struct RtreeResult {
    pub handle: Reference,
    pub x: Reference,
    pub y: Reference,
    pub z: Reference,
    pub radius: Reference,
    pub user: Reference,
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Rtree", module_name = "rtree")]
pub struct Rtree {
    #[intuicio(ignore)]
    tree: RTree<GeomWithData<Sphere, Integer>>,
    #[intuicio(ignore)]
    handle_generator: Integer,
}

#[intuicio_methods(module_name = "rtree")]
impl Rtree {
    #[intuicio_method(use_registry)]
    pub fn add(
        registry: &Registry,
        mut rtree: Reference,
        x: Reference,
        y: Reference,
        z: Reference,
        radius: Reference,
        user: Reference,
    ) -> Reference {
        let mut rtree = rtree.write::<Rtree>().expect("`rtree` is not an Rtree!");
        let x = *x.read::<Real>().expect("`x` is not a Real!");
        let y = *y.read::<Real>().expect("`y` is not a Real!");
        let z = *z.read::<Real>().expect("`z` is not a Real!");
        let radius = *radius.read::<Real>().expect("`radius` is not a Real!");
        let handle = rtree.handle_generator;
        rtree.handle_generator = rtree.handle_generator.wrapping_add_unsigned(1);
        rtree.tree.insert(GeomWithData::new(
            Sphere {
                x,
                y,
                z,
                radius,
                user,
            },
            handle,
        ));
        Reference::new_integer(handle, registry)
    }

    #[intuicio_method()]
    pub fn remove(mut rtree: Reference, handle: Reference) -> Reference {
        let mut rtree = rtree.write::<Rtree>().expect("`rtree` is not an Rtree!");
        let handle = *handle.read::<Integer>().expect("`handle` is not a Real!");
        let found = rtree.tree.iter().find(|item| item.data == handle).cloned();
        if let Some(found) = found {
            rtree.tree.remove(&found);
        }
        Reference::null()
    }

    #[intuicio_method()]
    pub fn clear(mut rtree: Reference) -> Reference {
        let mut rtree = rtree.write::<Rtree>().expect("`rtree` is not an Rtree!");
        rtree.tree = RTree::default();
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn nearest(
        registry: &Registry,
        rtree: Reference,
        x: Reference,
        y: Reference,
        z: Reference,
    ) -> Reference {
        let rtree = rtree.read::<Rtree>().expect("`rtree` is not an Rtree!");
        let x = *x.read::<Real>().expect("`x` is not a Real!");
        let y = *y.read::<Real>().expect("`y` is not a Real!");
        let z = *z.read::<Real>().expect("`z` is not a Real!");
        rtree
            .tree
            .nearest_neighbor(&[x, y, z])
            .map(|item| {
                Reference::new(
                    RtreeResult {
                        handle: Reference::new_integer(item.data, registry),
                        x: Reference::new_real(item.geom().x, registry),
                        y: Reference::new_real(item.geom().y, registry),
                        z: Reference::new_real(item.geom().z, registry),
                        radius: Reference::new_real(item.geom().radius, registry),
                        user: item.geom().user.clone(),
                    },
                    registry,
                )
            })
            .unwrap_or_default()
    }

    #[allow(clippy::too_many_arguments)]
    #[intuicio_method(use_context, use_registry)]
    pub fn visit(
        context: &mut Context,
        registry: &Registry,
        rtree: Reference,
        x: Reference,
        y: Reference,
        z: Reference,
        radius: Reference,
        callback: Reference,
    ) -> Reference {
        let rtree = rtree.read::<Rtree>().expect("`rtree` is not an Rtree!");
        let x = *x.read::<Real>().expect("`x` is not a Real!");
        let y = *y.read::<Real>().expect("`y` is not a Real!");
        let z = *z.read::<Real>().expect("`z` is not a Real!");
        let radius = if radius.is_null() {
            Real::INFINITY
        } else {
            *radius.read::<Real>().expect("`radius` is not a Real!")
        };
        let mut iterate = true;
        for item in rtree.tree.nearest_neighbor_iter(&[x, y, z]) {
            if !iterate {
                break;
            }
            if item.geom().distance_2(&[x, y, z]) > radius * radius {
                break;
            }
            let item = Reference::new(
                RtreeResult {
                    handle: Reference::new_integer(item.data, registry),
                    x: Reference::new_real(item.geom().x, registry),
                    y: Reference::new_real(item.geom().y, registry),
                    z: Reference::new_real(item.geom().z, registry),
                    radius: Reference::new_real(item.geom().radius, registry),
                    user: item.geom().user.clone(),
                },
                registry,
            );
            if let Some(function) = callback.read::<Function>() {
                context.stack().push(item);
                function.invoke(context, registry);
                if let Some(status) = context.stack().pop::<Reference>() {
                    if let Some(status) = status.read::<Boolean>() {
                        iterate = *status;
                    }
                }
            } else if let Some(closure) = callback.read::<Closure>() {
                let status = closure.invoke(context, registry, &[item]);
                if let Some(status) = status.read::<Boolean>().map(|status| *status) {
                    iterate = status;
                }
            }
        }
        Reference::null()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn version() -> IntuicioVersion {
    core_version()
}

#[unsafe(no_mangle)]
pub extern "C" fn install(registry: &mut Registry) {
    registry.add_type(Rtree::define_struct(registry));
    registry.add_type(RtreeResult::define_struct(registry));
    registry.add_function(Rtree::add__define_function(registry));
    registry.add_function(Rtree::remove__define_function(registry));
    registry.add_function(Rtree::clear__define_function(registry));
    registry.add_function(Rtree::nearest__define_function(registry));
    registry.add_function(Rtree::visit__define_function(registry));
}
