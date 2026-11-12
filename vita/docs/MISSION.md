# Mission Claude Code — Horizon Vita

Tu es responsable de créer un véritable port autonome du moteur Horizon pour PS Vita, distribuable en VPK, depuis le dossier Windows :
`G:\1_JEUX\Minecraft\AA mod\AA PS VITA`.

## Mission
Créer **Horizon Vita** : launcher natif PS Vita + client Rust d'Horizon exécutable LOCALLEMENT sur la PS Vita (aucun streaming depuis PC), basé sur le code original et inspiré du port Switch. Préparer les ressources depuis un Minecraft Java légalement acquis, puis les transférer sur Vita. Interface 960x544 adaptée aux sticks, boutons, écran tactile, sauvegardes, mods compatibles, packs et mises à jour Vita sûres. Pas de prétendue conversion automatique des versions PC.

## Sources à examiner réellement (ne pas supposer)
1. `https://github.com/horizon-rust/horizon` et `https://playhorizon.dev/docs` (source officielle Rust et restrictions).
2. `https://github.com/MakrSas/horizon-switch` ET dépôt présent localement dans `switch\switch` : lire `switch/README.md`, scripts, shim, patch crates, rendu, contrôles, architecture et licence ; NE RIEN EFFACER.
3. `https://github.com/megabytesme/MCHorizonWiiU` au commit `69fab597a6f0e8a6ce524d38d1b73e599e343034` : ne pas prétendre qu'il contient un vrai backend Wii U sans fichiers et build probants. Ce commit paraît être un fork desktop Horizon.
4. `https://dev.pgteam.org/main/mods/minecraft/enhanced` : seulement référence d'expérience Vita ; ne pas extraire de code propriétaire.
5. `https://github.com/vita-rust/examples`, `https://github.com/vita-rust/cargo-vita`, `https://github.com/vitasdk/samples`, `https://github.com/Rinnegatamante/vitaGL`.
6. `https://github.com/xerpi/vita-udcd-uvc`, `https://github.com/jeff20001-new/VitaSync`, `https://github.com/devnoname120/vitacompanion` pour la boucle écran / commandes / déploiement.

## Contraintes impératives
- Travailler dans la racine ci-dessus, créer des sous-dossiers propres ; préserver intégralement `switch\switch`, données personnelles et jeux installés.
- Ne pas écrire dans un dossier système Vita tant qu'un test et une sauvegarde ne sont pas faits. Chaque étape doit être réversible.
- Ne pas dire « fonctionnel » avant une compilation réelle ET un test sur une vraie Vita.
- Priorité à un VPK minimal stable, puis un client 3D Rust ; ne jamais masquer un écran noir ou inventer des FPS.
- Vita : ARMv7 32 bits Cortex-A9, RAM limitée, GPU PowerVR : port du renderer via sceGxm/vitaGL, pas de DX12/Vulkan desktop. `wgpu` original doit être isolé ou remplacé. Proposer alternative renderer GXM native si vitaGL est insuffisant.
- Rust Vita Tier 3 avec standard library partielle et `-Z build-std`; certaines crates ne compileront pas sans adaptation. Valider crate par crate avec cargo et des tests mesurés.
- Ne pas inclure fichiers Mojang/Microsoft ni clés, dumps de firmware, devkit officiels ni données utilisateurs dans un dépôt public ou VPK. Ne récupérer les assets que depuis une copie possédée par l'utilisateur.
- Mods Java `.jar` Fabric/Forge/NeoForge non exécutables par Horizon. `.hmod` contenu éventuellement ; `.hnmod` natif et shaders doivent être spécifiquement recompilés/compatibilisés pour PS Vita. Pas de promesse de shader PC sur Vita.
- Le moteur de base suit Minecraft Java 26.3 ; autres versions = uniquement si port Vita réellement testé et implémenté.
- Auto-updates : suivre commits Horizon en CI, reconstruire PORT VITA pour ARMv7 et exécuter tests. Le launcher Vita ne reçoit que des releases Vita signées et compatibles. Les mises à jour de données peuvent être indépendantes ; prévoir rollback et sauvegarde. Une version upstream Windows ne sera jamais téléchargée comme un VPK.
- Conserver LICENSE, avis des tiers et crédit du projet d'origine. Respecter les licences des composants GXM/vitaGL.

## Organisation technique
- `vita-bootstrap/` est un vrai squelette natif C/SDL2 et menu de diagnostic construit sur les exemples VitaSDK. Exécuter et TESTER le VPK. Le bouton Play ne doit PAS feindre un jeu.
- `rust-platform/` est un contrat Rust placeholder ; ne pas confondre avec le moteur. Il faut ajouter ensuite un binaire Vita Rust via `cargo-vita` et des modules `platform-vita` exploitables par Horizon.
- `vendor/` ou `third_party/` pour copies ou submodules des upstream verrouillés par commit, sans assets jeu.
- `ports/vita/` pour bindings, renderer GXM, contrôles, fichiers, audio, allocations, threading et networking.
- `tools/` pour WSL build, export assets légal, diagnostics, deploy, screenshot.
- `docs/` pour architecture, issue log, compatibilité, licences, budgets mémoire/VRAM, logs de tests.

## Phases de livraison, chacune avec PREUVES
A. Auditer les sources, afficher les versions commit et construire tableau de compatibilité (crates, dépendances, GPU, licences). Rapporter le statut Switch local réel.
B. Configurer Windows/WSL, VitaSDK, SDL2, Rust nightly, cargo-vita, cross target ; compiler `vita-bootstrap`, obtenir un vrai `*.vpk`; test sur console.
C. Prototyper un exécutable Rust Vita qui affiche un écran, reçoit les boutons et journalise ses allocations. C++/C shim acceptable si nécessaire, mais logique du client visé en Rust.
D. Construire un cube/chunks avec rendu GXM performant ; mesurer frame-time, RAM, VRAM et défauts. Faire un mode debug avec overlay FPS et mémoire.
E. Compiler les crates compatibles d'Horizon sans `wgpu`, isoler le rendu, adapter input/audio/FS/network ; charger une map minimaliste.
F. Préparer ressources sur PC à partir de Minecraft Java officiel ; importer sur Vita ; chargement d'un monde et sauvegarde reproductible.
G. Ajouter UX 960x544, sticks, tactile, profils graphiques, son, sauvegardes, mods contenu (après validation), gestion de packs et validation des shaders Vita.
H. Automatiser des RELEASES Vita vérifiées. Suivre l'upstream, détecter changements brisants et exiger CI + validation Vita avant diffusion. Mettre à jour sans perdre sauvegardes.
I. Intégrer une boucle de test PC / Vita : capture UDCD-UVC ou VitaSync, logs, FTP vitacompanion, capture de screenshots; contrôle automatisé par Claude uniquement si pont local explicitement autorisé et testé.

## Critères finaux
1. VPK installé, menu natif 960x544 et boutons Vita confirmés sur vraie console.
2. Démarrage et fermeture propres, logs et erreurs clairs, rien de supprimé.
3. Horizon Rust en local et réellement jouable (pas un écran décoratif, pas Minecraft Vita modifié, pas Moonlight), avec reproduction du test de sauvegarde.
4. Benchmarks documentés sur matériel réel ; aucune supposition sur FPS ou shader.
5. Releases Vita avec manifeste signé, SHA-256, rollback et politique de compatibilité.

**Commence sans poser des questions déjà résolues** : inspecte la racine locale, les sous-dossiers Switch et le code open source, et commence par compiler/valider le squelette. Si la console est absente, termine chaque étape vérifiable sur PC et indique clairement celles qui nécessitent le matériel. Ne déclare pas le moteur fonctionnel tant que ce n'est pas démontré. Réponds avec les chemins EXACTS des fichiers modifiés, les commandes exécutées, leurs sorties et les blocages réels. Ne saute pas les étapes critiques, mais continue à accomplir toutes les tâches possibles.

## Pont visuel optionnel préinstallé dans ce scaffold
Le script `tools/vita-bridge.py` permet une capture UVC (`capture`) et l'envoi explicite d'une touche (`key ... --allow-control`) vers une fenêtre VitaSync déjà ouverte. Tu peux lire la capture PNG pour analyser les écrans réels. Ne lance jamais le contrôle automatiquement sans autorisation de l'utilisateur, ne déclenche pas de commandes risquant de toucher le firmware ou la configuration des plugins, et documente les limites du prototype VitaSync.
