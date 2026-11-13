# Horizon Vita : état du portage, système par système

Mis à jour le 2026-11-13, application 0.0.7. Base : ce dépôt (fork Wii U, Horizon amont
`69fab597a`), branche `vita-port`.

Légende des colonnes :

- **Compile Vita** : `cargo check` pour `armv7-sony-vita-newlibeabihf` (types seulement),
  ou **lié** quand le code est réellement dans le VPK.
- **Sur console** : seulement ce qui a été exécuté sur une vraie PS Vita (PCH-1000),
  prouvé par photo ou par `ux0:data/HorizonVita/horizon.log`.

## Ce qui est du code Horizon

| Système | Code Horizon réutilisé | Adaptation Vita | Compile Vita | Sur console |
| --- | --- | --- | --- | --- |
| Bruit, aléatoire, positions | `horizon-core` | aucune | lié (0.0.4) | **oui** : résultat identique au PC (`F748DA62`) |
| Registre des blocs, modèles, sprites | `horizon-assets`, `horizon-terrain-mesh::assets` | aucune | lié (0.0.7) | non testé |
| Faits des blocs | `horizon-items::blocks` | aucune | lié (0.0.7) | non testé |
| Générateur de l'Overworld (relief, grottes, biomes) | `horizon-world` (build réduit `cafe-terrain-only` du port Wii U) | aucune | lié (0.0.7) | non testé |
| Décoration (arbres, minerais, plantes) | `horizon-world` | aucune | check OK ; non liée (build réduit) | non ; tourne sur PC pour le `.hvw` |
| Structures | `horizon-world` | aucune | check OK ; non liées | non ; tournent sur PC pour le `.hvw` |
| Mesher des chunks | `horizon-terrain-mesh` | `spans.rs` : pas d'horloge CPU par thread sur Vita | lié (0.0.7) | non testé |
| Adaptateur générateur → mesher | repris de `wiiu/worldgen` | réécrit dans `vita/world` | lié (0.0.7) | non testé |
| Sauvegardes (`level.dat`, régions) | `horizon-world::level_dat`, `horizon-client::save` | — | `horizon-world` check OK ; `horizon-client` échoue | non |
| HUD, atlas d'interface | `horizon-client-hud` | — | check OK | non |
| Écrans et widgets | `horizon-client-ui` | — | check OK | non |
| Entrées (clavier, manette) | `horizon-client-input` sans `desktop` | — | check OK | non ; le menu Vita lit les boutons lui-même |
| Police, PNG | `horizon-client-font` | — | check OK | non |
| Protocole réseau | `horizon-protocol` | — | check OK | non |
| Commandes | `horizon-command` | — | check OK | non |
| Joueur, physique, entités, IA, inventaire, crafting, fours, redstone, fluides en jeu, son | `horizon-client` (549 000 lignes) | — | **échoue** | non |
| Rendu desktop | `horizon-client-render` (`wgpu`) | à remplacer, pas à porter | **échoue** | non |

## Ce qui est propre au port Vita

| Élément | Où | Compile Vita | Sur console |
| --- | --- | --- | --- |
| Application unique (menu, dossiers, diagnostic) | `vita/engine/src/app.rs` | lié | **oui** (0.0.4) |
| Rendu GXM (perspective, profondeur, culling par lot) | `vita/engine/src/gfx.rs`, sur libvita2d | lié | **oui** (0.0.4, 0.0.5) |
| Affichage d'un monde Horizon préparé sur PC (`.hvw`) | `gfx.rs`, `vita/world-tools` | lié | **oui** (0.0.5) |
| Terrain de test (substitut, pas Horizon) | `vita/engine/src/terrain.rs` | lié | **oui** ; à retirer quand le vrai monde tourne |
| Pont PC (écran, état, journal, boutons, fichiers) | `vita/engine/src/bridge.rs`, `vita/bridge` | lié (0.0.6) | non testé ; protocole testé sur PC |
| Test Horizon sur console | `vita/engine/src/horizon_test.rs` | lié (0.0.7) | non testé |

## Ce qui bloque `horizon-client` (le jeu lui-même)

Erreurs de `cargo check -p horizon-client --no-default-features` sur la cible Vita :

| Dépendance | Pourquoi | Piste |
| --- | --- | --- |
| `wgpu`, `winit`, `naga` | rendu et fenêtre desktop, pas de Vulkan sur Vita | le port Wii U a commencé à séparer `Frame` et les contrats `PresentationBackend` / `TerrainBackend` : un backend GXM doit les implémenter |
| `ring` (via `ureq` + TLS) | assembleur sans support Vita | désactiver TLS pour le portage, ou le fork `vita-rust/ring` |
| `libmimalloc-sys` | allocateur desktop | fonction `mimalloc` déjà optionnelle dans `horizon-threads` |
| `memmap2`, `rustix` | API Unix absentes de la Vita | remplacer par des lectures de fichiers |
| `xxhash-rust` (44 erreurs, via `horizon-client-lib`, fonction `xxh3`) | son code NEON s'active dès que le processeur a NEON, mais il n'est écrit que pour l'ARM 64 bits ; le Cortex-A9 de la Vita a NEON en 32 bits | petit correctif : limiter ce code à `target_arch = "aarch64"` (copie corrigée dans `third_party`, comme les autres correctifs du dépôt) |

Aucune de ces dépendances n'est du gameplay : ce sont des couches système. C'est là que le
portage doit travailler, pas dans la logique du jeu.

## Mémoire

| Mesure | Valeur | Source |
| --- | ---: | --- |
| RAM libre au lancement (tas par défaut de 128 MiB déjà réservé) | 235 520 KiB | console, 0.0.4 |
| Mémoire vidéo libre au lancement | 114 688 KiB | console, 0.0.4 |
| Tas réservé par la 0.0.7 | 224 MiB demandés | à confirmer : ligne `C/Rust heap reserved` du journal |
| Pic du démarrage d'Horizon, build réduit, 32 bits | 159 MiB | PC, `phases` en i686 |
| Mémoire tenue ensuite (3x3 chunks générés et maillés) | 87 MiB | PC, `phases` en i686 |
| Pic du build complet (structures) en 32 bits | 501 MiB | PC : ne tient pas sur Vita |

Le pic vient de `BlockFacts::load`, qui lit `gameplay-facts.json` (55 Mo) entier puis
l'analyse deux fois. Un format compact préparé sur PC le supprimerait.
