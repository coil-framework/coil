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

class CmsEditorController extends Controller<HTMLElement> {
  connect() {
    this.updateSummary();
  }

  toggleBlock(event: Event) {
    const target = event.currentTarget;
    if (!(target instanceof HTMLButtonElement)) return;
    const card = target.closest<HTMLElement>("[data-block-card]");
    if (!card) return;
    const collapsed = card.dataset.collapsed === "true";
    card.dataset.collapsed = collapsed ? "false" : "true";
    card.classList.toggle("admin-card--collapsed", !collapsed);
    target.textContent = collapsed ? "Collapse" : "Expand";
  }

  expandAll() {
    this.setAllCollapsed(false);
  }

  collapseAll() {
    this.setAllCollapsed(true);
  }

  private setAllCollapsed(collapsed: boolean) {
    this.element.querySelectorAll<HTMLElement>("[data-block-card]").forEach((card) => {
      card.dataset.collapsed = collapsed ? "true" : "false";
      card.classList.toggle("admin-card--collapsed", collapsed);
      const button = card.querySelector<HTMLButtonElement>("[data-block-toggle]");
      if (button) {
        button.textContent = collapsed ? "Expand" : "Collapse";
      }
    });
    this.updateSummary();
  }

  private updateSummary() {
    const cards = Array.from(this.element.querySelectorAll<HTMLElement>("[data-block-card]"));
    const enabled = cards.filter((card) => card.dataset.blockEnabled === "true").length;
    const disabled = cards.length - enabled;
    const summary = this.element.querySelector<HTMLElement>("[data-cms-block-summary]");
    if (summary) {
      summary.textContent = `${cards.length} blocks, ${enabled} enabled, ${disabled} disabled`;
    }
  }
}

document.body.dataset.controller = [document.body.dataset.controller, "admin--interactive cms--editor"].filter(Boolean).join(" ");
const app = Application.start();
app.register("admin--interactive", AdminInteractiveController);
app.register("cms--editor", CmsEditorController);
