# PC / Claude Code / PS Vita

## Voir l'écran
- Option robuste et connue : `xerpi/vita-udcd-uvc`, plugin taiHEN `udcd_uvc.skprx`; la Vita apparaît comme webcam USB dans OBS (vidéo seulement, audio séparé via jack). Brancher la Vita modifiée au PC via câble data.
- Alternative intégrant vidéo et commandes : `jeff20001-new/VitaSync`. Il combine un plugin kernel `VitaSync.skprx` et une interface Windows. **Prototype non validé officiellement**, demander consentement, sauvegarder `ur0:tai/config.txt`, surveiller risques kernel et prévoir redémarrage avec L pour sauter les plugins.
- Ne pas activer simultanément des plugins kernel USB concurrents sans vérification.

## Commander
- Avec VitaSync, clavier/souris/manette sur le PC peuvent envoyer des commandes Vita (suivant l'état du prototype). La vidéo/capture n'est pas une API de contrôle IA prête à l'emploi.
- Pour permettre à Claude Code d'agir, il faut EN PLUS un pont explicite : outil MCP/desktop autorisé pour saisir dans la fenêtre VitaSync, une API propre au projet si disponible, ou un adaptateur de test de la future application exposant des commandes de debug dédiées. Ne pas prétendre que Claude Code voit l'écran de Windows sans ce pont.
- Tester par étapes : screenshot confirmé, puis un unique bouton, retour, logs et déconnexion. Contrôles distants OFF par défaut hors session développement.

## Déployer
- `vita-rust/cargo-vita` + `vitacompanion` permettent upload VPK / remplacement eboot sur homebrew déjà installé et re-lancement en mode dev. Pour VPK C/CMake, utiliser VitaShell/FTP avec vérification.
- `PrincessLog` pour logs; optionnel `VitaDebugger` pour GDB applicatif (sur LAN privé uniquement).
- Fournir à Claude Code les logs + captures dans `artifacts/local-only/`; ne pas commiter d'identifiants Microsoft, de dumps de firmware, de clés, d'assets propriétaires ou de saves personnelles.

## Pont local fourni (optionnel)
Depuis **Windows** avec Python 3 :
```powershell
python -m pip install opencv-python pyautogui pygetwindow
python tools/vita-bridge.py capture --camera 0
python tools/vita-bridge.py key enter --window-title VitaSync --allow-control
```
Les captures sont écrites dans `artifacts/local-only/vita-latest.png` et peuvent être ouvertes par Claude Code via ses outils de fichiers. Le numéro de caméra est à identifier dans OBS/Windows ; aucune commande n'est envoyée à la Vita si le plugin VitaSync ne fonctionne pas. Les touches doivent correspondre à la configuration VitaSync. Le pont ne contient aucun contrôle réseau et ne garantit pas un support HID complet.
