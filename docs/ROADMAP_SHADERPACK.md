# FEUILLE DE ROUTE STRATÉGIQUE : DÉCOUPLAGE DE `sunrise-render-shaderpack`

Cette feuille de route définit la séquence d'exécution pour achever l'extraction du pipeline Shader Pack d'Horizon / Sunrise hors du monolithe `sunrise-client-render` vers la crate dédiée `sunrise-render-shaderpack`.

---

## 📊 ÉTAT D'AVANCEMENT GLOBAL

| Module | Statut | Lignes | Emplacement actuel |
| :--- | :--- | :--- | :--- |
| `pack_timing` / `timing` | ✅ Extrait | ~1 850 l. | `sunrise-render-shaderpack` |
| `pack_atlas` / `stitch` | ✅ Extrait | ~1 280 l. | `sunrise-render-shaderpack` |
| `pack_draw`, `pack_lines`, `pack_images` | ✅ Extrait | ~450 l. | `sunrise-render-shaderpack` |
| `pack_shadow` | ✅ Extrait | ~300 l. | `sunrise-render-shaderpack` |
| `pack_weather` | ✅ Extrait | ~250 l. | `sunrise-render-shaderpack` |
| `pack_sky` | ✅ Extrait | ~470 l. | `sunrise-render-shaderpack` |
| `pack_center_depth` | ✅ Extrait | ~340 l. | `sunrise-render-shaderpack` |
| `pack_textures` | ✅ Extrait | ~410 l. | `sunrise-render-shaderpack` |
| `pack_hand` | ✅ Extrait | ~390 l. | `sunrise-render-shaderpack` |
| `pack_entities` | ✅ Extrait (Agent A) | ~1 870 l. | `sunrise-render-shaderpack` |
| `pack_uniforms` | ✅ Extrait (Agent B) | ~2 840 l. | `sunrise-render-shaderpack` |
| `pack_post` | ✅ **Extrait & stabilisé** (Agent 1/2 + clôture Agent 3) | ~6 490 l. | `sunrise-render-shaderpack` (+ pont client `PackPostExt`) |

> **État `pack_post` :** le monolithe `crates/sunrise-client-render/src/gfx/pack_post.rs` est supprimé.
> Les types, la construction et l'exécution vivent dans `sunrise-render-shaderpack/src/pack_post/`
> (`types`, `construction`, `execution`, `image_formats`, `packed`). Le module
> `crates/sunrise-client-render/src/gfx/pack_post/` ne conserve que les ré-exports `pub use` et le
> pont `PackPostExt`, qui traduit `Frame` / `SkinFrame` / `EntitySpan` en `UniformFrameView`,
> `SkyInputs` et `HandInputs` — c'est l'architecture canonique, ce pont reste **côté client**.
> Répartition des tests : 15 tests purs dans `sunrise-render-shaderpack`, 103 dans
> `sunrise-client-render::gfx::pack_post` (dont 6 `ignored`), soit 118 tests au total.

---

## 🎯 PHASAGE DE CLÔTURE DU CHANTIER

### 1. PHASE PRÉPARATOIRE : Agent 0 (Refactoring & Scission sur place)
- **Objectif :** Découper le monolithe de 6 500 lignes de `pack_post.rs` en sous-modules internes isolés au sein de `crates/sunrise-client-render/src/gfx/pack_post/`.
- **Règle absolue :** Invariance stricte du comportement, 0 régression, vérification locale et distante complète.
- **Livrable :**
  - `pack_post/types.rs` : Structures de données & énumérations.
  - `pack_post/construction.rs` : `PackPost::new()` et allocation des pipelines/textures.
  - `pack_post/execution.rs` : Boucle de passes `run()`, `prepare_entities()`, `shadow_view()`, `submitted()`.
  - `pack_post/mod.rs` : Structure `PackPost` et glue d'assemblage.

### 2. PHASE D'EXTRACTION FINALE : 3 Agents en Fan-Out
Grâce à la scission de la Phase 1, chaque agent dispose de son propre fichier sans collision git :
- **Agent 1 (Types & Core) :** Déplace `types.rs` vers `sunrise-render-shaderpack/src/pack_post/types.rs`.
- **Agent 2 (Pipelines & Runtime) :** Déplace `construction.rs` et `execution.rs` vers `sunrise-render-shaderpack`.
- **Agent 3 (Harnesses & Tests) :** Adapte et migre les ~40 fichiers de tests sous `crates/sunrise-client-render/src/gfx/pack_post/`.

### 3. PHASE DE FINALISATION & ATTERRISSAGE
- `crates/sunrise-client-render/src/gfx/pack_post/mod.rs` ré-exporte le crate
  (`pub use sunrise_render_shaderpack::pack_post::{construction,execution}::*`) et ajoute le pont
  `PackPostExt` ; `types.rs` demeure un shim `pub use` déclaré.
- Validation `tools/check_layers.py` (respect des couches architecturales).
- Validation `tools/pre_push_check.py --base <base> --skip-clippy`.
- Push sur `darkliro-sunrise` et ouverture de la PR finale vers `sunrise-org/sunrise:main`.

**État de clôture (Agent 3, commit `3da65b74ae`)** — vérifié sur
`/run/media/arthur/Data/horizone/horizon-shaderpack` :

- `cargo check -p sunrise-render-shaderpack -p sunrise-client-render --tests` : OK.
- `check_layers.py` : 82 crates, 26 exceptions restantes.
- `format_scan.py` : propre. `pre_push_check.py --base 3b2cc30a14 --skip-clippy` : 7/7 gates OK.
- ⚠️ La gate « unused imports » de `pre_push_check.py` invoque `dark-run`, qui synchronise
  `horizon-main` (autre arbre) : la vérifier localement avec
  `RUSTFLAGS='-D unused_imports' cargo check -p sunrise-render-shaderpack -p sunrise-client-render --lib --no-default-features`.
