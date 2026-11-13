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

## 7. Rendu 3D matériel (`vita/engine`, 2026-11-12)

État : **compilé et empaqueté, pas encore lancé sur console.**

- Rendu : GXM natif. `libvita2d` (MIT, `a8f15ab09`) fournit le contexte GXM, les tampons
  d'affichage, le tampon de profondeur S8D24 et son shader couleur précompilé
  (`position * wvp`). Le moteur lui passe sa propre matrice de perspective et ses propres
  tampons de sommets et d'indices mappés pour le GPU. Aucun compilateur de shaders n'est
  requis sur la console (pas de `libshacccg.suprx`, contrairement à vitaGL).
- Limite connue : vita2d n'a pas de shader « texture x couleur par sommet ». Les textures
  demanderont un shader à nous, donc un moyen de le compiler.
- Scène : 8x8 chunks de 16x16 colonnes, hauteurs issues du bruit Perlin de `horizon-core`.
  Le découpage en colonnes et le mesher sont un substitut écrit pour ce test : ce n'est ni
  le générateur de monde d'Horizon ni `horizon-terrain-mesh`.
- Référence PC, graine 20261112 : checksum `DF77845F`, 64 chunks, 104 384 sommets,
  52 192 triangles.
- Mesures affichées et écrites dans `ux0:data/HorizonVita/engine.log` : FPS, durée de frame,
  temps CPU, attente GPU (`sceGxmFinish`), chunks dessinés après culling, RAM et VRAM libres.
- Commandes : sticks, L/R, Croix (graine suivante), Triangle (vsync), Carré (culling),
  Rond (mesure GPU), START.

## 8. Erreur d'installation 0xFFFFFFFF (résolue côté empaquetage)

`cargo vita build vpk` (cargo-vita 0.2.2) écrit sous Windows des noms d'entrées zip avec des
antislashs (`sce_sys\icon0.png`). VitaShell refuse alors le VPK avec `0xFFFFFFFF`.
`vita/tools/build-rust-vpk.ps1` compile l'eboot avec cargo-vita puis empaquette lui-même
avec des `/`, et vérifie les noms. L'installation du VPK corrigé reste à confirmer sur console.

## 9. Une seule application (`HorizonVita.vpk`, 0.0.4)

`vita/engine` regroupe désormais tout dans un seul binaire Rust, Title ID `HRZN00001` :
menu, JOUER (le test de rendu 3D de la section 7), MODS (dossiers), DIAGNOSTIC (commandes,
tactile, mémoire, auto-test du bruit `horizon-core` comparé à la valeur du PC `F748DA62`).
Le menu et le rendu partagent la même boucle de frames : pas de second programme à lancer.
Journal : `ux0:data/HorizonVita/horizon.log`. `vita/bootstrap` (C/SDL2) et `vita/rust-probe`
restent dans le dépôt comme références, ils ne sont plus distribués.

État : compilé et empaqueté ; **jamais lancé sur console**.

Construction et copie :

```
vita/tools/build-rust-vpk.ps1 -Crate engine -TitleId HRZN00001 -AppName "Horizon Vita" -AppVer 00.04 -OutName HorizonVita -VitaSdk <sdk>
vita/tools/deploy-vita.ps1        # copie vers F:\vpk si la carte est montée
```

## 10. 0xFFFFFFFF sur tous les VPK : cause probable côté VitaShell

Après le VPK aux antislashs (section 8), les trois VPK suivants, pourtant bien formés
(noms, CRC et tailles vérifiés entre en-têtes locaux et répertoire central), ont échoué avec
la même erreur. Lecture du code de VitaShell (`81af709`) :

- `0xFFFFFFFF` (-1) ne sort que de la lecture d'archive (`archiveOpen`, `archiveFileOpen`,
  `archiveFileRead`), avant toute installation ;
- `archiveFileOpen` renvoie -1 si une archive est « déjà ouverte » (`archive_fd` non nul) ;
- `extractArchiveFile` appelle `psarcFileClose` au lieu de `archiveFileClose` quand la
  création du fichier de destination échoue : `archive_fd` reste alors non nul. C'est ce qui
  arrive avec un nom contenant un antislash.

Conséquence attendue : après un échec de ce type, VitaShell refuse **tous** les VPK jusqu'à
sa fermeture complète. Le mode USB ne le redémarre pas. Hypothèse cohérente avec les faits
(le bootstrap 0.0.3 échoue alors que le 0.0.2, de même structure, s'était installé), mais
**non confirmée** tant qu'une installation n'a pas réussi après redémarrage de VitaShell.

Suite (2026-11-12, 22h59 heure console) : `HorizonVita.vpk`, bien formé, a encore été refusé
avec `0xFFFFFFFF`. Il n'est pas établi que VitaShell avait été fermé entre-temps. L'application
`HRZN00001` n'est plus dans `ux0:app` (désinstallée côté console). `deploy-vita.ps1` dépose
maintenant aussi le contenu du VPK en dossier (`ux0:vpk/HorizonVita-dossier`) : l'option
« Installer le dossier » de VitaShell n'utilise pas son lecteur d'archives, ce qui permet de
trancher entre « archive refusée » et « autre cause ».

## 11. Test sur console du 0.0.4 (2026-11-12, PCH-1000)

Confirmé par photos et par `ux0:data/HorizonVita/horizon.log` :

- L'installation a réussi après fermeture complète de VitaShell : l'hypothèse de la
  section 10 est confirmée.
- Menu, dossiers, diagnostic : fonctionnent. Police système chargée.
- Calcul Horizon : bruit `F748DA62` et monde de test `DF77845F`, **identiques au PC**.
  Le calcul du bruit (32 640 échantillons, 3 octaves) prend 99,8 ms sur la console.
- Rendu 3D GXM : perspective, profondeur, caméra aux sticks et culling fonctionnent.

Mesures (terrain de test, couleurs unies, 52 192 triangles au total) :

| Mesure | Valeur |
| --- | ---: |
| RAM libre avant graphismes / après | 235 520 / 225 280 KiB |
| Mémoire vidéo libre avant / après | 114 688 / 91 136 KiB |
| Génération + maillage + envoi du terrain de test | 51 + 26 + 25 ms |
| CPU par frame | ~2,0 ms |
| GPU, 45 000 triangles à l'écran | 23 à 24 ms (36 à 39 FPS) |
| GPU, 28 000 à 35 000 triangles | 14 à 15 ms (58 à 60 FPS) |

Le rendu est limité par le GPU. Causes probables, non encore mesurées séparément : pas de
suppression des faces arrière, et le shader de vita2d mélange par alpha, ce qui empêche le
GPU d'écarter les surfaces cachées.

## 12. Le vrai Horizon : coût mesuré sur PC, et ce qui en découle

`vita/world-tools`, binaire `phases`, sur les ressources préparées par le launcher
(26.3, 1 301 sprites de 64 px) :

| Étape | Temps PC | Mémoire après | Pic |
| --- | ---: | ---: | ---: |
| `Assets::load` | 0,05 s | 11 MiB | 24 MiB |
| `reload_for_terrain` (modèles, sprites) | 0,56 s | 62 MiB | 107 MiB |
| `BlockFacts::load` | 0,16 s | 70 MiB | 186 MiB |
| `load_visibility`, `MeshTables`, `Biomes` | 0,12 s | 87 MiB | 186 MiB |
| `GeneratorData::load`, générateur | 0,10 s | 92 MiB | 186 MiB |

Générer et mailler ensuite 9 chunks prend 0,06 s. Le coût est donc au démarrage :
`BlockFacts::load` analyse `gameplay-facts.json` (55 Mo), deux fois. Avec 225 MiB libres
et un processeur bien plus lent, charger ces ressources brutes sur la console n'est pas
raisonnable : il faudra un format compact préparé sur PC. Non commencé.

## 13. Monde Horizon emballé sur PC (0.0.5)

Étape intermédiaire, pour voir le vrai Minecraft sur la console avant d'y porter le
chargement :

- `vita/world-tools`, binaire `pack` : le générateur 26.3 d'Horizon (relief, grottes,
  structures, puis décoration : 2 437 éléments placés, 0 non pris en charge sur 49 chunks)
  et le mesher d'Horizon tournent **sur le PC**. Le résultat est écrit dans un fichier
  `.hvw` : un atlas et des quads texturés.
- `preview` : rendu logiciel du `.hvw` en PNG, avec les mêmes matrices et le même ordre de
  dessin que la console. Contrôlé à l'œil : herbe, terre, plantes, eau corrects.
- La console lit `ux0:data/HorizonVita/world/world.hvw` et l'affiche avec le shader
  texture précompilé de vita2d. Sans ce fichier, JOUER affiche le terrain de test.

Référence (graine 20261112, rayon 2) : 25 chunks, 77 994 quads (66 037 opaques, 10 601
découpés, 1 356 translucides), 317 lots, 130 cellules d'atlas, atlas 1024x64, 6,2 MiB.

Limites, toutes dues au shader disponible (une texture, pas de couleur par sommet) :

- la teinte et l'ombrage de chaque quad sont cuits dans l'atlas : éclairage plat, pas
  d'occlusion ambiante lissée ;
- tout est éclairé par le ciel, grottes comprises : le moteur de lumière n'est pas branché ;
- les textures découpées (feuilles, herbes) sont mélangées par alpha et écrivent la
  profondeur : des trous peuvent apparaître derrière elles ;
- sprites réduits à 16 px, pas d'animation ;
- rien n'est généré, chargé ni modifiable sur la console : ce n'est pas jouable.

État : VPK 0.0.5 compilé, fichier monde copié sur la carte ; **pas encore lancé sur console**.
Le fichier monde dérive du Minecraft de l'utilisateur : il n'est ni versionné ni distribué.

## 14. Test sur console du 0.0.5 (2026-11-13)

Le monde Horizon emballé s'affiche, avec les textures du joueur. Photo et journal :

| Vue | Triangles dessinés | GPU | FPS |
| --- | ---: | ---: | ---: |
| Presque rien à l'écran | 4 000 à 5 000 | 13,4 ms | 60 |
| Vue moyenne | 33 000 à 35 000 | 14 à 18 ms | 50 à 58 |
| Horizon entier à l'écran | 80 000 à 102 000 | 47 à 54 ms | 18 à 20 |

CPU : 2,5 à 2,8 ms par frame. Le GPU limite. Le coût d'environ 13 ms même avec très peu de
triangles indique un coût fixe par frame (remplissage de l'écran ou mélange alpha), pas
seulement le nombre de triangles. Le culling des faces arrière (SELECT) n'a pas été essayé.

## 15. Pont PC (0.0.6) et test Horizon sur console (0.0.7)

Pont : serveur TCP dans l'application (port 4747), désactivé par défaut, activable par le
menu PONT PC. Commandes : état, journal, capture d'écran, boutons, caméra, réglages,
envoi et lecture de fichiers sous `ux0:data/HorizonVita` uniquement (chemins avec `..`
refusés). Client PC : `vita/bridge` (`vita-bridge`). Le protocole complet a été testé sur
PC contre le même code serveur (mode `bridge` de l'application compilée pour PC) :
toutes les commandes, transfert de 6,2 MiB identique à l'aller et au retour, chemin
`../evil.txt` refusé. **Pas encore testé sur console.**

Test Horizon : DIAGNOSTIC, Triangle (ou `vita-bridge <ip> run horizon`). Il charge les
ressources préparées copiées dans `ux0:data/HorizonVita/resources/26.3` (151 Mo, sans
sons ni langues), puis génère et maille 1 puis 3x3 chunks avec le vrai code d'Horizon,
build réduit du port Wii U. Références PC (même code, 64 bits) :

| Étape | Temps PC | Résultat |
| --- | ---: | --- |
| Chargement complet | 1,0 s | 1 301 sprites de 64 px, apparition (0, 0) |
| 1 chunk | 0,05 s | 2 689 quads, somme du maillage `6CD47A79`, surface 80 |
| 3x3 chunks | 0,12 s | 24 007 quads, somme du maillage `D33FE9FC` |

La console devra donner les mêmes sommes. La 0.0.7 demande un tas de 224 MiB
(`_newlib_heap_size_user`) au lieu de 128 MiB par défaut, pour le pic mesuré de 159 MiB
en 32 bits. **Compilée et liée (VPK de 3,7 Mo), pas encore lancée sur console.**

Tableau système par système : `PORT_STATUS.md`.
