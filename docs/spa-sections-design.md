# SPA Sections Architecture & Compatibility Design

## Overview
The website operates as a single-page application (SPA) containing major logical navigation sections (e.g., `#about-us`, `#our-works`, `#festivals`, `#gallery`, `#contact-us`). Each SPA section contains zero or more content blocks (`text`, `text_image`, `text_youtube`, `text_video`).

## Hierarchy Model
```
Page (e.g. slug = 'home')
  ↓
SpaSection (e.g. section_key = 'about-us')
  ↓
ContentBlock (physical PostgreSQL `sections` table)
  ↓
optional MediaAsset (via `section_media` relation)
```

## Physical PostgreSQL Table Compatibility
To preserve schema integrity without breaking existing relations, the physical PostgreSQL table names are structured as follows:
- **`spa_sections`** (NEW): Represents logical SPA navigation sections on a page.
- **`sections`** (EXISTING): Logically represents **ContentBlock** entities. A mandatory `spa_section_id` foreign key references `spa_sections(id)`.
- **`section_media`** (EXISTING): Maps a `ContentBlock` (`sections`) to its attached `MediaAsset`.

## First-Class Domain Ownership vs. Arbitrary Tags
Arbitrary tags (such as `tags: ["about-us"]`) were **explicitly rejected** because:
1. Tags do not provide database-enforced relational integrity or foreign key constraints.
2. Tags do not allow explicit ordering, visibility toggling (`is_visible`), or localized navigation metadata (`navigation_label`) per section.
3. Logical SPA sections are first-class entities (`SpaSection`), ensuring every active content block belongs to exactly one SPA section.

## Navigation & Block Ordering Semantics
- **SPA Navigation Order**: `spa_sections.sort_order` (e.g. 10, 20, 30, 40, 50).
- **Block Order Within SPA Section**: `sections.sort_order` (0, 10, 20, ...).

## Visibility Invariants
- `SpaSection.is_visible` & `ContentBlock.is_visible`.
- A content block is publicly visible **only if** both its owning `SpaSection` and the content block itself have `is_visible = TRUE`.
