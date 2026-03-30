import "@hotwired/turbo";
import { Application, Controller } from "@hotwired/stimulus";

class AdminInteractiveController extends Controller<HTMLElement> {
  connect() {
    this.bindFilters();
    this.bindCopyButtons();
  }

  private bindFilters() {
    this.element.querySelectorAll<HTMLElement>("[data-admin-filter]").forEach((scope) => {
      const input = scope.querySelector<HTMLInputElement>("[data-admin-filter-input]");
      if (!input) return;

      const applyFilter = () => {
        const query = input.value.trim().toLowerCase();
        scope.querySelectorAll<HTMLElement>("[data-admin-filter-item]").forEach((item) => {
          const matches = item.textContent?.toLowerCase().includes(query) ?? false;
          item.toggleAttribute("hidden", !matches);
        });
      };

      input.addEventListener("input", applyFilter);
      applyFilter();
    });
  }

  private bindCopyButtons() {
    this.element.querySelectorAll<HTMLButtonElement>("[data-copy-text]").forEach((button) => {
      button.addEventListener("click", async () => {
        const value = button.dataset.copyText;
        if (!value) return;
        try {
          await navigator.clipboard.writeText(value);
          const original = button.textContent;
          button.textContent = "Copied";
          window.setTimeout(() => {
            if (original) button.textContent = original;
          }, 1200);
        } catch {
          button.textContent = "Copy failed";
        }
      });
    });
  }
}

document.body.dataset.controller = [document.body.dataset.controller, "admin--interactive"].filter(Boolean).join(" ");
const app = Application.start();
app.register("admin--interactive", AdminInteractiveController);
