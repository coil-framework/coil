import "@hotwired/turbo";
import { Application, Controller } from "@hotwired/stimulus";

const PRODUCT_GALLERY_IMAGES: Record<string, [string, string][]> = {
  "harbor-cap": [
    ["https://unsplash.com/photos/a-rack-of-shirts-and-pants-hanging-on-a-clothes-rack-1pT3rOWL_hI/download?force=true&w=1200&q=80", "Shop floor rail"],
    ["https://unsplash.com/photos/a-storefront-with-clothes-displayed-inside-Sd_vsr_eA5U/download?force=true&w=1200&q=80", "Storefront detail"],
    ["https://unsplash.com/photos/people-browsing-clothing-racks-in-a-well-lit-store-oOAYziRlpMw/download?force=true&w=1200&q=80", "Browsing detail"]
  ],
  "gold-membership": [
    ["https://unsplash.com/photos/woman-in-colorful-outfit-and-fur-coat-DxSHu4GI0Ao/download?force=true&w=1200&q=80", "Campaign portrait"],
    ["https://unsplash.com/photos/woman-in-white-dress-choosing-coat-from-fur-coats-oyLU7C-2kRE/download?force=true&w=1200&q=80", "Wardrobe mood"],
    ["https://unsplash.com/photos/elegant-woman-in-a-faux-fur-coat-poses-VaCpUIoNIeE/download?force=true&w=1200&q=80", "Editorial detail"]
  ],
  "tasting-pass": [
    ["https://unsplash.com/photos/clothing-store-interior-with-racks-of-apparel-a-_PeeYVfQk/download?force=true&w=1200&q=80", "Paris edit"],
    ["https://unsplash.com/photos/modern-clothing-store-interior-with-colorful-garments-on-display-DS7N9ZnKpO0/download?force=true&w=1200&q=80", "Store interior"],
    ["https://unsplash.com/photos/modern-retail-store-interior-with-displays-and-lighting-yKENxnOwxhg/download?force=true&w=1200&q=80", "Retail lighting"]
  ],
  "harbor-scarf": [
    ["https://unsplash.com/photos/woman-wearing-gray-coat-CKxpOhAoSRg/download?force=true&w=1200&q=80", "Winter styling"],
    ["https://unsplash.com/photos/grayscale-photography-of-woman-wearing-pea-coat-Zq4dVEMAZXo/download?force=true&w=1200&q=80", "Coat detail"],
    ["https://unsplash.com/photos/woman-in-black-coat-by-a-close-door-lBabHA3imdk/download?force=true&w=1200&q=80", "Cold weather mood"]
  ],
  "brooklyn-night-pass": [
    ["https://unsplash.com/photos/modern-luxury-store-interior-with-display-shelves-and-seating-8YDqTT5jNXI/download?force=true&w=1200&q=80", "Night edit interior"],
    ["https://unsplash.com/photos/modern-retail-store-interior-with-display-cases-and-lighting-CUxuy9UmFIo/download?force=true&w=1200&q=80", "Event mood"],
    ["https://unsplash.com/photos/modern-retail-store-interior-with-display-shelves-and-products-lkDZJL5psKU/download?force=true&w=1200&q=80", "Display detail"]
  ]
};

class SiteInteractiveController extends Controller<HTMLElement> {
  private carouselIntervalId: number | null = null;

  connect() {
    this.setupPanelToggles();
    this.setupCarousel();
    this.setupAccordions();
    this.setupSizePicker();
    this.setupGallery();
  }

  disconnect() {
    if (this.carouselIntervalId !== null) {
      window.clearInterval(this.carouselIntervalId);
    }
  }

  private setupPanelToggles() {
    this.element.querySelectorAll<HTMLElement>("[data-panel-toggle]").forEach((button) => {
      button.addEventListener("click", () => {
        const panelId = button.getAttribute("data-panel-toggle");
        const panel = panelId ? document.getElementById(panelId) : null;
        if (!panel) return;

        const isOpen = !panel.hasAttribute("hidden");
        document.querySelectorAll<HTMLElement>(".switcher-panel").forEach((entry) => entry.setAttribute("hidden", ""));
        document
          .querySelectorAll<HTMLElement>("[data-panel-toggle]")
          .forEach((entry) => entry.setAttribute("aria-expanded", "false"));

        if (!isOpen) {
          panel.removeAttribute("hidden");
          button.setAttribute("aria-expanded", "true");
        }
      });
    });

    document.addEventListener("click", (event) => {
      const target = event.target;
      if (!(target instanceof HTMLElement)) return;
      if (target.closest(".switcher-panel") || target.closest("[data-panel-toggle]")) return;
      document.querySelectorAll<HTMLElement>(".switcher-panel").forEach((entry) => entry.setAttribute("hidden", ""));
      document
        .querySelectorAll<HTMLElement>("[data-panel-toggle]")
        .forEach((entry) => entry.setAttribute("aria-expanded", "false"));
    });
  }

  private setupCarousel() {
    const carousel = this.element.querySelector<HTMLElement>("[data-carousel='hero']");
    if (!carousel) return;
    const slides = Array.from(carousel.querySelectorAll<HTMLElement>("[data-carousel-slide]"));
    if (slides.length < 2) return;

    let index = slides.findIndex((slide) => slide.classList.contains("is-active"));
    if (index < 0) {
      index = 0;
      slides[0]?.classList.add("is-active");
    }

    const show = (nextIndex: number) => {
      index = (nextIndex + slides.length) % slides.length;
      slides.forEach((slide, slideIndex) => {
        slide.classList.toggle("is-active", slideIndex === index);
      });
    };

    carousel.querySelector<HTMLElement>("[data-carousel-prev]")?.addEventListener("click", () => show(index - 1));
    carousel.querySelector<HTMLElement>("[data-carousel-next]")?.addEventListener("click", () => show(index + 1));

    if (!window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      this.carouselIntervalId = window.setInterval(() => show(index + 1), 7000);
    }
  }

  private setupAccordions() {
    this.element.querySelectorAll<HTMLElement>(".accordion-trigger").forEach((button) => {
      button.addEventListener("click", () => {
        const panel = button.nextElementSibling;
        const isExpanded = button.getAttribute("aria-expanded") === "true";
        button.setAttribute("aria-expanded", String(!isExpanded));
        if (panel instanceof HTMLElement) {
          panel.hidden = isExpanded;
          panel.classList.toggle("is-open", !isExpanded);
        }
      });
    });
  }

  private setupSizePicker() {
    this.element.querySelectorAll<HTMLElement>(".size-picker").forEach((picker) => {
      picker.querySelectorAll<HTMLElement>(".size-pill").forEach((button) => {
        button.addEventListener("click", () => {
          picker.querySelectorAll<HTMLElement>(".size-pill").forEach((entry) => {
            entry.classList.remove("is-active");
            entry.setAttribute("aria-pressed", "false");
          });
          button.classList.add("is-active");
          button.setAttribute("aria-pressed", "true");
        });
      });
    });
  }

  private setupGallery() {
    this.element.querySelectorAll<HTMLElement>(".product-gallery").forEach((gallery) => {
      const key = gallery.getAttribute("data-gallery-key");
      const image = gallery.querySelector("[data-gallery-image]");
      if (!(image instanceof HTMLImageElement) || !key) return;

      const variants = PRODUCT_GALLERY_IMAGES[key] || [];
      gallery.querySelectorAll<HTMLElement>(".gallery-thumb").forEach((button, index) => {
        let source = button.getAttribute("data-gallery-src");
        let alt = button.getAttribute("data-gallery-alt");
        if ((!source || !alt) && variants[index]) {
          [source, alt] = variants[index];
        }
        if (!source || !alt) return;

        button.addEventListener("click", () => {
          image.src = source!;
          image.alt = alt!;
          gallery.querySelectorAll<HTMLElement>(".gallery-thumb").forEach((entry) => entry.classList.remove("is-active"));
          button.classList.add("is-active");
        });
      });
    });
  }
}

document.body.dataset.controller = [document.body.dataset.controller, "site--interactive"].filter(Boolean).join(" ");
const app = Application.start();
app.register("site--interactive", SiteInteractiveController);
