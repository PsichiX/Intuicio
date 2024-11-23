// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded "><a href="index.html"><strong aria-hidden="true">1.</strong> Introduction</a></li><li class="chapter-item expanded "><a href="scripting-pipeline-explained.html"><strong aria-hidden="true">2.</strong> Scripting pipeline explained</a></li><li class="chapter-item expanded "><a href="anatomy-of-a-host.html"><strong aria-hidden="true">3.</strong> Anatomy of a Host</a></li><li class="chapter-item expanded "><a href="anatomy-of-a-script.html"><strong aria-hidden="true">4.</strong> Anatomy of a Script</a></li><li class="chapter-item expanded "><a href="tutorial/index.html"><strong aria-hidden="true">5.</strong> Tutorial</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="tutorial/frontend.html"><strong aria-hidden="true">5.1.</strong> Building custom frontend</a></li><li class="chapter-item expanded "><a href="tutorial/backend.html"><strong aria-hidden="true">5.2.</strong> Building custom backend</a></li><li class="chapter-item expanded "><a href="tutorial/runner.html"><strong aria-hidden="true">5.3.</strong> Building custom runner</a></li></ol></li><li class="chapter-item expanded "><a href="official-frontends/index.html"><strong aria-hidden="true">6.</strong> Official frontends</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="official-frontends/assembler/index.html"><strong aria-hidden="true">6.1.</strong> Assembler</a></li><li class="chapter-item expanded "><a href="official-frontends/serde/index.html"><strong aria-hidden="true">6.2.</strong> Serde</a></li><li class="chapter-item expanded "><a href="official-frontends/vault/index.html"><strong aria-hidden="true">6.3.</strong> Vault</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/index.html"><strong aria-hidden="true">6.4.</strong> Simpleton</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="official-frontends/simpleton/language-reference.html"><strong aria-hidden="true">6.4.1.</strong> Language reference</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/index.html"><strong aria-hidden="true">6.4.2.</strong> API reference</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/reflect.html"><strong aria-hidden="true">6.4.2.1.</strong> Reflect</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/closure.html"><strong aria-hidden="true">6.4.2.2.</strong> Closure</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/debug.html"><strong aria-hidden="true">6.4.2.3.</strong> Debug</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/console.html"><strong aria-hidden="true">6.4.2.4.</strong> Console</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/math.html"><strong aria-hidden="true">6.4.2.5.</strong> Math</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/array.html"><strong aria-hidden="true">6.4.2.6.</strong> Array</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/map.html"><strong aria-hidden="true">6.4.2.7.</strong> Map</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/text.html"><strong aria-hidden="true">6.4.2.8.</strong> Text</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/bytes.html"><strong aria-hidden="true">6.4.2.9.</strong> Bytes</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/json.html"><strong aria-hidden="true">6.4.2.10.</strong> JSON</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/toml.html"><strong aria-hidden="true">6.4.2.11.</strong> TOML</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/iter.html"><strong aria-hidden="true">6.4.2.12.</strong> Iterators</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/event.html"><strong aria-hidden="true">6.4.2.13.</strong> Event</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/promise.html"><strong aria-hidden="true">6.4.2.14.</strong> Promise</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/fs.html"><strong aria-hidden="true">6.4.2.15.</strong> File system</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/process.html"><strong aria-hidden="true">6.4.2.16.</strong> Process</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/net.html"><strong aria-hidden="true">6.4.2.17.</strong> Network</a></li><li class="chapter-item expanded "><a href="official-frontends/simpleton/api-reference/jobs.html"><strong aria-hidden="true">6.4.2.18.</strong> Jobs (multithreading)</a></li></ol></li></ol></li></ol></li><li class="chapter-item expanded "><a href="official-runners/index.html"><strong aria-hidden="true">7.</strong> Official runners</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="official-runners/simpleton/index.html"><strong aria-hidden="true">7.1.</strong> Simpleton</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="official-runners/simpleton/api-reference/index.html"><strong aria-hidden="true">7.1.1.</strong> API reference</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="official-runners/simpleton/api-reference/script.html"><strong aria-hidden="true">7.1.1.1.</strong> Script</a></li></ol></li></ol></li><li class="chapter-item expanded "><a href="official-runners/alchemyst/index.html"><strong aria-hidden="true">7.2.</strong> Alchemyst</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="official-runners/alchemyst/api-reference/index.html"><strong aria-hidden="true">7.2.1.</strong> API reference</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="official-runners/alchemyst/api-reference/color.html"><strong aria-hidden="true">7.2.1.1.</strong> Color</a></li><li class="chapter-item expanded "><a href="official-runners/alchemyst/api-reference/vec2.html"><strong aria-hidden="true">7.2.1.2.</strong> Vec2</a></li><li class="chapter-item expanded "><a href="official-runners/alchemyst/api-reference/image.html"><strong aria-hidden="true">7.2.1.3.</strong> Image</a></li><li class="chapter-item expanded "><a href="official-runners/alchemyst/api-reference/image-pipeline.html"><strong aria-hidden="true">7.2.1.4.</strong> Image Pipeline</a></li></ol></li></ol></li></ol></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString();
        if (current_page.endsWith("/")) {
            current_page += "index.html";
        }
        var links = Array.prototype.slice.call(this.querySelectorAll("a"));
        var l = links.length;
        for (var i = 0; i < l; ++i) {
            var link = links[i];
            var href = link.getAttribute("href");
            if (href && !href.startsWith("#") && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The "index" page is supposed to alias the first chapter in the book.
            if (link.href === current_page || (i === 0 && path_to_root === "" && current_page.endsWith("/index.html"))) {
                link.classList.add("active");
                var parent = link.parentElement;
                if (parent && parent.classList.contains("chapter-item")) {
                    parent.classList.add("expanded");
                }
                while (parent) {
                    if (parent.tagName === "LI" && parent.previousElementSibling) {
                        if (parent.previousElementSibling.classList.contains("chapter-item")) {
                            parent.previousElementSibling.classList.add("expanded");
                        }
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', function(e) {
            if (e.target.tagName === 'A') {
                sessionStorage.setItem('sidebar-scroll', this.scrollTop);
            }
        }, { passive: true });
        var sidebarScrollTop = sessionStorage.getItem('sidebar-scroll');
        sessionStorage.removeItem('sidebar-scroll');
        if (sidebarScrollTop) {
            // preserve sidebar scroll position when navigating via links within sidebar
            this.scrollTop = sidebarScrollTop;
        } else {
            // scroll sidebar to current active section when navigating via "next/previous chapter" buttons
            var activeSection = document.querySelector('#sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        var sidebarAnchorToggles = document.querySelectorAll('#sidebar a.toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(function (el) {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define("mdbook-sidebar-scrollbox", MDBookSidebarScrollbox);
