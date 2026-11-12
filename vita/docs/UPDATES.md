# Mises à jour : contrat de sécurité

**Jamais** télécharger et lancer directement les exécutables Windows/Linux/Switch du projet upstream Horizon. Pas de promesse d'auto-portage : le moteur doit être reconstruit et testé pour PS Vita.

Deux flux :
- **Données compatibles** : paramètres, fichiers index, packs de ressources autorisés. Peut être actualisé sans recompiler le VPK lorsque schémas et capacités concordent.
- **Binaire Vita** : publication de `HorizonVita.vpk` ou d'un contenu installé signé et validé par une CI Vita. Upgrade uniquement via mécanisme d'installation Vita vérifié ; ne pas écraser à chaud `eboot.bin` pendant son exécution.

Une release valide contient `channel`, `vita_build_version`, `upstream_commit`, `min_bootstrap_version`, `title_id`, `sha256`, `size`, `signature`, `url`, `compatibility` et les changements de migrations. Le canal Vita (non upstream) est la source de vérité. Prévoir manifest signé, allowlist de domaine, validation SHA-256 ET signature, téléchargement temporaire, reprise, limite taille, espace disque, backup des mondes, rollback et bouton annuler. Aucune clé privée sur la console.

`examples/update-manifest.example.json` n'est qu'un schéma d'illustration. Il ne pointe sur aucun serveur ni binaire et ne doit pas déclencher une mise à jour.
