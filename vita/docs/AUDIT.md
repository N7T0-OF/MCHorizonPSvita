# Horizon Vita : audit et journal de tests

État au 2026-11-12. **Le moteur Horizon n'est pas porté.** Ce qui existe : un menu de
diagnostic (C/SDL2) testé sur console, un probe Rust compilé mais pas encore testé sur
console, et une vérification de type des crates Horizon pour la cible Vita.

## 1. Sources auditées

| Source | Commit | Licence | Constat |
| --- | --- | --- | --- |
| `horizon-rust/horizon` | `ba91f75cb` (HEAD) | MIT | Amont officiel, Minecraft Java 26.3, rendu `wgpu`. |
| `megabytesme/MCHorizonWiiU` | `63cf4b0b4` (HEAD) | MIT | Base de ce dépôt. `69fab597a` est l'amont pur ; HEAD ajoute 102 commits et un dossier `wiiu/`. Pas de client jouable : son `PORTING.md` dit qu'aucun menu ni aucune frame Horizon n'a été vérifié. |
| `MakrSas/horizon-switch` | `ff30ca80c` | MIT | Port Switch : `wgpu` conservé au-dessus de Vulkan (NVK). Non transposable : la Vita n'a pas Vulkan. |
| `vita-rust/examples` | `5ef9e08e1` | MIT ou Apache-2.0 | Base du framebuffer et de la police du probe Rust. |
| `vita-rust/cargo-vita` | `c6bbf22d8` (0.2.2 installé) | MIT ou Apache-2.0 | Empaquetage VPK depuis cargo. |
| `vitasdk/samples` | `fe8fbef57` | aucune licence dans le dépôt | Référence seulement, rien de copié. |
| `Rinnegatamante/vitaGL` | `e1473ffa0` | LGPL-3.0 | Candidat pour le rendu ; lien statique = obligations LGPL à respecter. |
| `xerpi/vita-udcd-uvc` | `973fb652b` | aucune licence dans le dépôt | Capture écran USB ; non installé. |
| `jeff20001-new/VitaSync` | `6cebd0c07` | aucune licence dans le dépôt | Prototype écran + commandes ; non installé. |
| `devnoname120/vitacompanion` | `3ebee0ab5` | MIT | FTP et commandes de déploiement ; non installé. |

`AA PS VITA\switch\switch` n'est pas du code source : c'est une installation Switch
(`horizon.nro` et ressources Minecraft préparées). Rien n'y a été modifié et rien n'en est
versionné. Son `log.txt` montre un processus de 3 189 MiB ; son `panic.txt` un crash
`wgpu` « Invalid surface ».

## 2. Outils (Windows natif, sans WSL)

- VitaSDK `master-win-v2.540` (GCC 15.2.0), SDL2 2.32.8, dans `AA PS VITA\_toolchain\vitasdk`.
- Rust `1.102.0-nightly (3ef0c22cd 2026-11-11)` avec `rust-src`, `cargo-vita 0.2.2`.
- `vita/tools/build-vpk.ps1` : bootstrap C. `cargo +nightly vita build vpk --release` : probe Rust
  (définir `VITASDK` et `CARGO_TARGET_ARMV7_SONY_VITA_NEWLIBEABIHF_LINKER`).

## 3. Test sur console (2026-11-12, PS Vita PCH-1000, bootstrap 0.0.2)

Vérifié sur photos et sur `ux0:data/HorizonVita/bootstrap.log` :

- installation du VPK, bulle LiveArea, lancement ;
- menu 960x544, navigation haut/bas, Croix, Rond ;
- page diagnostic : sticks au repos 124/135 et 133/130, tactile avant et arrière lus ;
- rendu SDL : `VITA gxm` ; frame mesurée 22 ms (avec un `SDL_Delay(16)` volontaire) ;
- écriture de fichier sur `ux0:` : OK.

Mémoire libre au démarrage (après les 32 MiB de tas réservés par le bootstrap) :

| Zone | Libre |
| --- | ---: |
| RAM utilisateur | 207 872 KiB |
| CDRAM (mémoire vidéo) | 92 160 KiB |
| PHYCONT | 26 624 KiB |

Non vérifié : sortie par START (le log ne contient pas `clean exit`), bootstrap 0.0.3,
probe Rust.

## 4. Crates Horizon sur la cible `armv7-sony-vita-newlibeabihf`

`vita/tools/check-crates.ps1`, résultats bruts dans `crate-check-results.txt`.
**C'est un `cargo check` : types seulement, ni édition de liens ni exécution.**

| Crate | Résultat | Remarque |
| --- | --- | --- |
| `horizon-core` | OK | Seule crate aussi liée et empaquetée en VPK (probe). |
| `horizon-terrain-data`, `horizon-hud-data` | OK | |
| `horizon-threads` (sans mimalloc) | OK | |
| `horizon-assets`, `horizon-items`, `horizon-command` | OK | |
| `horizon-world` | OK | Génération de monde et sauvegardes. |
| `horizon-client-core`, `horizon-client-platform`, `horizon-client-input` (sans desktop) | OK | |
| `horizon-resource-zip`, `horizon-client-font`, `horizon-shaders` | OK | |
| `horizon-terrain-mesh` (sans mimalloc) | OK après correctif | `spans.rs` : pas d'horloge CPU par thread sur Vita. |
| `horizon-client-hud`, `horizon-client-ui` | OK après le même correctif | |
| `horizon-protocol` | OK | |
| `horizon-client-render` | ÉCHEC | `ring`, `memmap2`, `rustix`, `wgpu` : c'est le rendu desktop. |
| `horizon-client` | ÉCHEC | Dépend du rendu et de la fenêtre desktop. |

Ce qui manque est donc la couche hôte : rendu (GXM ou vitaGL), boucle d'événements,
audio, et le remplacement de `memmap2`/`ring`.

## 5. Obstacles mesurés

1. **Mémoire.** ~240 MiB utilisables contre 3 189 MiB sur Switch. Le projet Wii U a mesuré
   un atlas de blocs de 2048x8192 RGBA (64 MiB), un atlas HUD de 4096x6144 (96 MiB) et une
   cuisson des modèles restée à ~599 MiB. Aucun ne tient dans 92 MiB de CDRAM.
2. **Données préparées.** `facts.json` 10 Mo, `gameplay-facts.json` 55 Mo, `cached-collision.nbt`
   5,7 Mo dans l'installation Switch : à charger en JSON, cela dépasse le budget. Il faudra
   un format binaire réduit, préparé sur PC.
3. **Rendu.** Pas de Vulkan : le rendu `wgpu` ne peut pas être gardé.
4. **32 bits.** Rien n'a encore tourné sur console à part `horizon-core` (à confirmer par le probe).

## 6. Mods

- Les `.jar` Fabric/Forge/NeoForge ne s'exécutent pas dans Horizon, sur aucune plateforme.
- Les mods Horizon sont du Rust compilé en WebAssembly : il faudra un interpréteur WASM qui
  tienne dans le budget mémoire de la Vita. Non évalué.
- Le bootstrap 0.0.3 crée `ux0:data/HorizonVita/{mods,resourcepacks,shaderpacks,saves}` et
  affiche leur chemin et leur contenu. Rien n'est chargé.
- Téléchargement Modrinth/CurseForge depuis la console : non commencé. Ne servirait qu'aux
  packs de ressources et de données tant que le moteur ne tourne pas.
