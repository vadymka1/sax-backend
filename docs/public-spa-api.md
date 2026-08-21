# Public SPA API Contract for React

## 1. Overview
The public backend exposes a single, aggregated endpoint for frontend rendering:

```http
GET /api/v1/public/page
```

This endpoint returns the complete structure of the application (sections, content blocks, and attached media) for the `home` page in a single HTTP request, eliminating N+1 queries on the client side.

---

## 2. Response JSON Contract

### HTTP Status: `200 OK`
Unauthenticated request. No `Authorization` header required.

```json
{
  "data": {
    "page": {
      "id": "813876e5-42d8-4fbb-91ea-72223a3bc990",
      "slug": "home",
      "title": "Home",
      "seo_title": null,
      "seo_description": null,
      "seo_keywords": null
    },
    "sections": [
      {
        "id": "e0b9687e-e2e4-4d8b-9a84-9343ee6df322",
        "key": "about-us",
        "title": "About Us",
        "navigation_label": "About Us",
        "sort_order": 10,
        "blocks": [
          {
            "id": "7649fb99-702b-47e1-b4cb-d48e89f81d19",
            "block_type": "text",
            "title": "Who We Are",
            "text": "SPA Saxophone Ensemble delivers world-class musical performances.",
            "media": null,
            "sort_order": 10
          }
        ]
      },
      {
        "id": "a90b4d45-6677-4933-bc89-112233445566",
        "key": "partners",
        "title": "Our Partners",
        "navigation_label": "Partners",
        "sort_order": 60,
        "blocks": []
      }
    ]
  }
}
```

---

## 3. Frontend Integration Rules

### 3.1 Navigation Generation
Frontend navigation items are generated directly from `data.sections`:
```javascript
const navItems = data.sections.map(section => ({
  label: section.navigation_label,
  href: `#${section.key}`
}));
```

### 3.2 Anchoring and Stable Keys
- Each section container should render `id={section.key}` (e.g. `<section id={section.key}>`).
- Dynamic section key allocations (`key`) remain immutable when an Admin modifies `title` or `navigation_label`.

### 3.3 Visibility Rules
- **Hidden Sections**: `SpaSection.is_visible = false` hides the section and ALL child blocks. Hidden sections are omitted entirely from `data.sections`.
- **Visible Empty Sections**: `SpaSection.is_visible = true` with 0 visible blocks is returned as `"blocks": []` so that newly created Admin sections appear immediately in navigation.
- **Hidden Blocks**: `ContentBlock.is_visible = false` hides only that specific block.

### 3.4 Media Representations
- `text`: `media = null`
- `text_image`: `media` object with `"type": "image"`, `"url": "/uploads/filename.jpg"`
- `text_video`: `media` object with `"type": "video"`, `"url": "/uploads/filename.mp4"`
- `text_youtube`: `media` object with `"type": "youtube"`, `"youtube_video_id"`, `"embed_url"`, `"thumbnail_url"`

No raw filesystem storage paths or internal database metadata are exposed.
