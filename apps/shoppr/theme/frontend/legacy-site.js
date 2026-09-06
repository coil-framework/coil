const PRODUCT_GALLERY_IMAGES = {
  "harbor-cap": [
    ["/theme/assets/images/a-rack-of-shirts-and-pants-hanging-on-a-clothes-rack-1pT3rOWL_hI.jpg", "Shop floor rail"],
    ["/theme/assets/images/a-storefront-with-clothes-displayed-inside-Sd_vsr_eA5U.jpg", "Storefront detail"],
    ["/theme/assets/images/people-browsing-clothing-racks-in-a-well-lit-store-oOAYziRlpMw.jpg", "Browsing detail"],
  ],
  "gold-membership": [
    ["/theme/assets/images/woman-in-colorful-outfit-and-fur-coat-DxSHu4GI0Ao.jpg", "Campaign portrait"],
    ["/theme/assets/images/woman-in-white-dress-choosing-coat-from-fur-coats-oyLU7C-2kRE.jpg", "Wardrobe mood"],
    ["/theme/assets/images/elegant-woman-in-a-faux-fur-coat-poses-VaCpUIoNIeE.jpg", "Editorial detail"],
  ],
  "tasting-pass": [
    ["/theme/assets/images/clothing-store-interior-with-racks-of-apparel-a-_PeeYVfQk.jpg", "Paris edit"],
    ["/theme/assets/images/modern-clothing-store-interior-with-colorful-garments-on-display-DS7N9ZnKpO0.jpg", "Store interior"],
    ["/theme/assets/images/modern-retail-store-interior-with-displays-and-lighting-yKENxnOwxhg.jpg", "Retail lighting"],
  ],
  "harbor-scarf": [
    ["/theme/assets/images/woman-wearing-gray-coat-CKxpOhAoSRg.jpg", "Winter styling"],
    ["/theme/assets/images/grayscale-photography-of-woman-wearing-pea-coat-Zq4dVEMAZXo.jpg", "Coat detail"],
    ["/theme/assets/images/woman-in-black-coat-by-a-close-door-lBabHA3imdk.jpg", "Cold weather mood"],
  ],
  "brooklyn-night-pass": [
    ["/theme/assets/images/modern-luxury-store-interior-with-display-shelves-and-seating-8YDqTT5jNXI.jpg", "Night edit interior"],
    ["/theme/assets/images/modern-retail-store-interior-with-display-cases-and-lighting-CUxuy9UmFIo.jpg", "Event mood"],
    ["/theme/assets/images/modern-retail-store-interior-with-display-shelves-and-products-lkDZJL5psKU.jpg", "Display detail"],
  ],
};

function setupPanelToggles() {
  document.querySelectorAll("[data-panel-toggle]").forEach((button) => {
    button.addEventListener("click", () => {
      const panelId = button.getAttribute("data-panel-toggle");
      const panel = document.getElementById(panelId);
      if (!panel) {
        return;
      }
      const isOpen = !panel.hasAttribute("hidden");
      document.querySelectorAll(".switcher-panel").forEach((entry) => entry.setAttribute("hidden", ""));
      document.querySelectorAll("[data-panel-toggle]").forEach((entry) => entry.setAttribute("aria-expanded", "false"));
      if (!isOpen) {
        panel.removeAttribute("hidden");
        button.setAttribute("aria-expanded", "true");
      }
    });
  });

  document.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement)) {
      return;
    }
    if (target.closest(".switcher-panel") || target.closest("[data-panel-toggle]")) {
      return;
    }
    document.querySelectorAll(".switcher-panel").forEach((entry) => entry.setAttribute("hidden", ""));
    document.querySelectorAll("[data-panel-toggle]").forEach((entry) => entry.setAttribute("aria-expanded", "false"));
  });
}

function setupCarousel() {
  const carousel = document.querySelector("[data-carousel='hero']");
  if (!(carousel instanceof HTMLElement)) {
    return;
  }
  const slides = Array.from(carousel.querySelectorAll("[data-carousel-slide]"));
  if (slides.length < 2) {
    return;
  }

  let index = slides.findIndex((slide) => slide.classList.contains("is-active"));
  if (index < 0) {
    index = 0;
    slides[0].classList.add("is-active");
  }

  const show = (nextIndex) => {
    index = (nextIndex + slides.length) % slides.length;
    slides.forEach((slide, slideIndex) => {
      slide.classList.toggle("is-active", slideIndex === index);
    });
  };

  carousel.querySelector("[data-carousel-prev]")?.addEventListener("click", () => show(index - 1));
  carousel.querySelector("[data-carousel-next]")?.addEventListener("click", () => show(index + 1));

  if (!window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    window.setInterval(() => show(index + 1), 7000);
  }
}

function setupAccordions() {
  document.querySelectorAll(".accordion-trigger").forEach((button) => {
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

function setupSizePicker() {
  document.querySelectorAll(".size-picker").forEach((picker) => {
    picker.querySelectorAll(".size-pill").forEach((button) => {
      button.addEventListener("click", () => {
        picker.querySelectorAll(".size-pill").forEach((entry) => {
          entry.classList.remove("is-active");
          entry.setAttribute("aria-pressed", "false");
        });
        button.classList.add("is-active");
        button.setAttribute("aria-pressed", "true");
      });
    });
  });
}

function setupGallery() {
  document.querySelectorAll(".product-gallery").forEach((gallery) => {
    const key = gallery.getAttribute("data-gallery-key");
    const image = gallery.querySelector("[data-gallery-image]");
    if (!(image instanceof HTMLImageElement) || !key) {
      return;
    }
    const variants = PRODUCT_GALLERY_IMAGES[key] || [];
    gallery.querySelectorAll(".gallery-thumb").forEach((button, index) => {
      let source = button.getAttribute("data-gallery-src");
      let alt = button.getAttribute("data-gallery-alt");
      if ((!source || !alt) && variants[index]) {
        source = variants[index][0];
        alt = variants[index][1];
      }
      if (!source || !alt) {
        return;
      }
      button.addEventListener("click", () => {
        image.src = source;
        image.alt = alt;
        gallery.querySelectorAll(".gallery-thumb").forEach((entry) => entry.classList.remove("is-active"));
        button.classList.add("is-active");
      });
    });
  });
}

document.addEventListener("DOMContentLoaded", () => {
  setupPanelToggles();
  setupCarousel();
  setupAccordions();
  setupSizePicker();
  setupGallery();
});
