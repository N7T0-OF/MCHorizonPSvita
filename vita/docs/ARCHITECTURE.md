# Architecture cible

## Séparer ce qui se met à jour de ce qui doit être recompilé

```
Horizon Vita (VPK installé, Title ID propre)
  - UI Vita / gamepad / tactile
  - client de mise à jour de données (HTTPS + vérification d'intégrité et signature)
  - environnement console Rust : filesystem, sockets, audio, timing
  - moteur Horizon compilé ARMv7 avec backends adaptés
      - horizon-core, horizon-world, horizon-protocol, horizon-items...
      - renderer Vita GXM / vitaGL au lieu de wgpu/Vulkan/DX12
      - chargement d'assets préparés sur PC depuis Minecraft légalement acquis
  - plugins: content mods compatibles et resource packs avec budgets mémoire
  - profils par version TESTÉE du port Vita uniquement
```

Le port Switch a du code et des adaptations libnx, NVK, winit, wgpu et shim ARM64 qui ne sont pas transposables à l'identique vers le PowerVR de la Vita. Réemployer les structures d'abstraction, le débogage, le traitement des événements, les règles de compilation, pas ses exécutables ni son renderer NVK.

### Risques bloquants
1. **GPU** : pas de Vulkan / DirectX12 / wgpu desktop fonctionnel sur Vita. Nouveau renderer GXM/vitaGL et pipeline shaders spécifique.
2. **Architecture** : Cortex-A9 ARMv7 32 bits contre les cibles desktop 64 bits. FFI et dépendances Rust à remplacer/feature-gater ; Rust target Vita Tier 3.
3. **Mémoire** : PS Vita 512 Mo RAM physiques, part système importante. Mesurer mémoire réellement disponible, heap/VRAM, chunks, maillages, allocs et pics. Limites par profil, sans nombre inventé.
4. **Fonctions Rust std** : support partiel sur Vita, `std::process` indisponible. Le launcher PC GPUI ne peut pas être compilé tel quel.
5. **Mods et shaders** : les .jar Forge/Fabric ne sont pas exécutables sur Horizon même sur PC ; les mods Rust natifs et shaders WGSL doivent être revalidés/portés pour ARMv7/GXM.
6. **Versions** : Horizon official joue Minecraft Java 26.3 à la date de l'audit. Ne pas promettre arbitrairement toutes versions vanilla.
7. **Assets** : produire légalement depuis les fichiers acquis par l'utilisateur, sur PC, puis copier sur Vita. Ne jamais inclure jar, textures ou assets Mojang dans le VPK public.
8. **Mise à jour** : chaque version upstream Horizon ne sera pas automatiquement exécutable. La chaîne Vita nécessite compilation/certification propres, tests de régression et publication du manifeste Vita.

### Priorités de réutilisation
- Commencer avec `horizon-core`, `horizon-world` et `horizon-protocol` en compilation croisée isolée pour dresser un tableau des dépendances ; ne pas partir de `horizon-client` monolithique.
- Mettre en place une boucle affichage/input/audio/fichiers sûre avant le chargement du monde.
- Expérimenter sur un monde minimal/micro terrain avec textures de démonstration libres ou générées, avant les assets du jeu.
- Séparer renderer et logique, en s'appuyant sur les interfaces du port Switch, mais sans forcer `wgpu` sur Vita.
- Gérer la RAM par budget et profils 2–4 chunks exploratoires (à ajuster après mesures).

### Critères de succès
S0: projet configure et produit un VPK, écran et boutons fonctionnels sur appareil réel.
S1: démonstration Rust `std`+core, stockage et affichage GPU 3D (triangle/cube) sur Vita.
S2: port de 2+ crates de logique + tests de parité exécutés sur PC et tests ciblés Vita.
S3: world chunk miniature et sauvegarde stable.
S4: survival créatif, inventaire, sonore, ressources propres.
S5: mods contenu, profils et mises à jour sûres.
S6: stabilisation, compatibilité saves, réseau si possible.

Les S1–S6 restent NON LIVRÉS.
