# Shoppr Townhouse: Brand Direction & Customer Journey

## 1. Product/Brand Direction Note

Shoppr has evolved from a technical "multi-site demo" into a believable, premium retail experience: **Shoppr Townhouse**.

The creative direction shifts the storefront to feel like a modern, design-forward urban lifestyle brand. Instead of generic placeholder copy pointing out features ("Layered interactivity", "Developer-visible"), the storefront now uses elevated, intentional merchandising language ("The Spring Edit", "Harbor Capsule", "Curated for you").

The visual language (via updated `site.css` and template classes) is sharper and cleaner. We reduced the bubbly border radii (from 28px to 16px), removed the heavy radial gradients in favor of clean, solid off-whites (`#faf9f7`), and standardized the use of a stark `#111` accent color. The result is a storefront that feels like a real commercial destination—one that prioritizes desirability, merchandising clarity, and conversion confidence over simply proving that the underlying framework works.

## 2. Updated Storefront Components

We successfully updated the customer-facing surfaces without touching the core runtime, extending the SDK, or leaking backend mechanisms into the templates:
- **Translation Catalogs (`en-GB.toml`, `fr-FR.toml`, `pl-PL.toml`)**: Rewritten completely to use premium retail copy (e.g., "Add to bag", "Shopping Bag", "Proceed to checkout") instead of "demo" text.
- **`home.html`**: Transformed from a feature showcase into a brand flagship homepage. It now leads with seasonal campaigns ("The Spring Edit") and uses editorial rails to introduce physical spaces ("Our Spaces") and the brand story ("Shoppr Journal").
- **`collection-grid.html` & `collection-detail.html`**: Re-framed as "The Edit" and "Seasonal Capsules", offering curated views of the catalog rather than flat lists.
- **`product-detail.html`**: Updated the layout and copy to prioritize product storytelling ("Details & Care", "In-Store Availability") and cleaner conversion paths ("Complete the look").
- **`cart.html`**: Elevated the bag review experience with sharper typography and more reassuring summary text.
- **`site.css`**: Applied the `shoppr-shell--elevated` design system, refining typography weights, background contrast, and interactive hover states.

## 3. Customer Journeys the Experience Sells

The new storefront is designed to support a cohesive, high-trust retail journey:
1. **Discovery & Aspiration**: A customer lands on the UK flagship, greeted by high-quality campaign imagery and the promise of free shipping. The homepage clearly establishes the brand's premium positioning.
2. **Curated Browsing**: Instead of hitting a wall of products, the customer is guided through "Edits" (e.g., The Harbor Capsule, Spring Essentials). This makes the catalog feel deliberate and styled.
3. **Confident Conversion**: On the product page, the customer sees clear details about materials, in-store availability, and related items. The transition to the "Shopping Bag" feels seamless, reassuring, and premium.
4. **Post-Purchase Loyalty**: The account dashboard isn't just a receipt list; it's a hub for managing physical event tickets and tracking Harbor Circle status, keeping the customer engaged between purchases.

## 4. Native Integration of Memberships & Events

Previously, Memberships and Events felt like separate, bolted-on "features." The new design integrates them directly into the commercial narrative:

- **Memberships as "Harbor Circle"**: Membership is no longer sold as a generic "Tier"; it is positioned as an exclusive club ("Harbor Circle") that grants early access to limited capsules and invitations to private events. It feels like a genuine commercial differentiator that a premium brand would offer.
- **Events as "Community & Spaces"**: Events are merchandised as in-store experiences ("Visit the Townhouse"). They appear alongside editorial content (e.g., styling workshops, product launches), making them a believable part of the brand's physical retail strategy rather than a random booking module.

By aligning the language and presentation, Memberships and Events now actively contribute to the brand's desirability rather than distracting from it.
