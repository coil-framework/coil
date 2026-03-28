const SHOPPR_SITES = [
  { id: "shoppr-uk", label: "United Kingdom", host: "uk.127.0.0.1.nip.io:8080" },
  { id: "shoppr-fr", label: "France", host: "fr.127.0.0.1.nip.io:8080" },
  { id: "shoppr-pl", label: "Poland", host: "pl.127.0.0.1.nip.io:8080" },
];

const SHOPPR_LOCALES = [
  { id: "en-GB", label: "English" },
  { id: "fr-FR", label: "Francais" },
  { id: "pl-PL", label: "Polski" },
];

const PRODUCT_GALLERY_IMAGES = {
  "harbor-cap": [
    ["https://unsplash.com/photos/a-rack-of-shirts-and-pants-hanging-on-a-clothes-rack-1pT3rOWL_hI/download?force=true&w=1200&q=80", "Shop floor rail"],
    ["https://unsplash.com/photos/a-storefront-with-clothes-displayed-inside-Sd_vsr_eA5U/download?force=true&w=1200&q=80", "Storefront detail"],
    ["https://unsplash.com/photos/people-browsing-clothing-racks-in-a-well-lit-store-oOAYziRlpMw/download?force=true&w=1200&q=80", "Browsing detail"],
  ],
  "gold-membership": [
    ["https://unsplash.com/photos/woman-in-colorful-outfit-and-fur-coat-DxSHu4GI0Ao/download?force=true&w=1200&q=80", "Campaign portrait"],
    ["https://unsplash.com/photos/woman-in-white-dress-choosing-coat-from-fur-coats-oyLU7C-2kRE/download?force=true&w=1200&q=80", "Wardrobe mood"],
    ["https://unsplash.com/photos/elegant-woman-in-a-faux-fur-coat-poses-VaCpUIoNIeE/download?force=true&w=1200&q=80", "Editorial detail"],
  ],
  "tasting-pass": [
    ["https://unsplash.com/photos/clothing-store-interior-with-racks-of-apparel-a-_PeeYVfQk/download?force=true&w=1200&q=80", "Paris edit"],
    ["https://unsplash.com/photos/modern-clothing-store-interior-with-colorful-garments-on-display-DS7N9ZnKpO0/download?force=true&w=1200&q=80", "Store interior"],
    ["https://unsplash.com/photos/modern-retail-store-interior-with-displays-and-lighting-yKENxnOwxhg/download?force=true&w=1200&q=80", "Retail lighting"],
  ],
  "harbor-scarf": [
    ["https://unsplash.com/photos/woman-wearing-gray-coat-CKxpOhAoSRg/download?force=true&w=1200&q=80", "Winter styling"],
    ["https://unsplash.com/photos/grayscale-photography-of-woman-wearing-pea-coat-Zq4dVEMAZXo/download?force=true&w=1200&q=80", "Coat detail"],
    ["https://unsplash.com/photos/woman-in-black-coat-by-a-close-door-lBabHA3imdk/download?force=true&w=1200&q=80", "Cold weather mood"],
  ],
  "brooklyn-night-pass": [
    ["https://unsplash.com/photos/modern-luxury-store-interior-with-display-shelves-and-seating-8YDqTT5jNXI/download?force=true&w=1200&q=80", "Night edit interior"],
    ["https://unsplash.com/photos/modern-retail-store-interior-with-display-cases-and-lighting-CUxuy9UmFIo/download?force=true&w=1200&q=80", "Event mood"],
    ["https://unsplash.com/photos/modern-retail-store-interior-with-display-shelves-and-products-lkDZJL5psKU/download?force=true&w=1200&q=80", "Display detail"],
  ],
};

function currentLocale() {
  const segment = window.location.pathname.split("/").filter(Boolean)[0];
  return SHOPPR_LOCALES.find((locale) => locale.id === segment)?.id || "en-GB";
}

function replaceLocale(pathname, locale) {
  const parts = pathname.split("/").filter(Boolean);
  if (parts.length && SHOPPR_LOCALES.some((entry) => entry.id === parts[0])) {
    parts[0] = locale;
  } else {
    parts.unshift(locale);
  }
  return `/${parts.join("/")}`;
}

function renderSwitcherPanels() {
  const marketPanel = document.getElementById("market-panel");
  const localePanel = document.getElementById("locale-panel");
  if (!marketPanel || !localePanel) {
    return;
  }

  const locale = currentLocale();
  const currentHost = window.location.host;
  const currentPath = window.location.pathname;

  marketPanel.innerHTML = [
    '<p class="switcher-panel__eyebrow">Market</p>',
    "<ul>",
    ...SHOPPR_SITES.map((site) => {
      const href = `${window.location.protocol}//${site.host}${replaceLocale(currentPath, locale)}`;
      const active = currentHost === site.host ? ' class="is-active"' : "";
      return `<li${active}><a href="${href}">${site.label}</a></li>`;
    }),
    "</ul>",
  ].join("");

  localePanel.innerHTML = [
    '<p class="switcher-panel__eyebrow">Language</p>',
    "<ul>",
    ...SHOPPR_LOCALES.map((item) => {
      const href = `${window.location.protocol}//${window.location.host}${replaceLocale(currentPath, item.id)}`;
      const active = locale === item.id ? ' class="is-active"' : "";
      return `<li${active}><a href="${href}">${item.label}</a></li>`;
    }),
    "</ul>",
  ].join("");
}

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
  renderSwitcherPanels();
  setupPanelToggles();
  setupCarousel();
  setupAccordions();
  setupSizePicker();
  setupGallery();
});
