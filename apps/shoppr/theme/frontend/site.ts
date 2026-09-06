import "@hotwired/turbo";
import { Application, Controller } from "@hotwired/stimulus";

const PRODUCT_GALLERY_IMAGES: Record<string, [string, string][]> = {
  "harbor-cap": [
    ["/theme/assets/images/a-rack-of-shirts-and-pants-hanging-on-a-clothes-rack-1pT3rOWL_hI.jpg", "Shop floor rail"],
    ["/theme/assets/images/a-storefront-with-clothes-displayed-inside-Sd_vsr_eA5U.jpg", "Storefront detail"],
    ["/theme/assets/images/people-browsing-clothing-racks-in-a-well-lit-store-oOAYziRlpMw.jpg", "Browsing detail"]
  ],
  "gold-membership": [
    ["/theme/assets/images/woman-in-colorful-outfit-and-fur-coat-DxSHu4GI0Ao.jpg", "Campaign portrait"],
    ["/theme/assets/images/woman-in-white-dress-choosing-coat-from-fur-coats-oyLU7C-2kRE.jpg", "Wardrobe mood"],
    ["/theme/assets/images/elegant-woman-in-a-faux-fur-coat-poses-VaCpUIoNIeE.jpg", "Editorial detail"]
  ],
  "tasting-pass": [
    ["/theme/assets/images/clothing-store-interior-with-racks-of-apparel-a-_PeeYVfQk.jpg", "Paris edit"],
    ["/theme/assets/images/modern-clothing-store-interior-with-colorful-garments-on-display-DS7N9ZnKpO0.jpg", "Store interior"],
    ["/theme/assets/images/modern-retail-store-interior-with-displays-and-lighting-yKENxnOwxhg.jpg", "Retail lighting"]
  ],
  "harbor-scarf": [
    ["/theme/assets/images/woman-wearing-gray-coat-CKxpOhAoSRg.jpg", "Winter styling"],
    ["/theme/assets/images/grayscale-photography-of-woman-wearing-pea-coat-Zq4dVEMAZXo.jpg", "Coat detail"],
    ["/theme/assets/images/woman-in-black-coat-by-a-close-door-lBabHA3imdk.jpg", "Cold weather mood"]
  ],
  "brooklyn-night-pass": [
    ["/theme/assets/images/modern-luxury-store-interior-with-display-shelves-and-seating-8YDqTT5jNXI.jpg", "Night edit interior"],
    ["/theme/assets/images/modern-retail-store-interior-with-display-cases-and-lighting-CUxuy9UmFIo.jpg", "Event mood"],
    ["/theme/assets/images/modern-retail-store-interior-with-display-shelves-and-products-lkDZJL5psKU.jpg", "Display detail"]
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
