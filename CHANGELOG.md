# Notes de version

Une section par version, la plus récente en tête. Une ligne `- ` par
changement, en mots d'utilisateur ; `[Windows]` ou `[Linux]` devant ce qui ne
concerne qu'une plateforme.

## 0.0.8 — 2026-11-17

- Sauvegarde automatique : vos ajouts (textes, images) sont mis en lieu sûr toutes les minutes, sans toucher au fichier ; l'heure de la dernière copie de secours s'affiche en bas de la fenêtre.
- Si PdfEditor se ferme brutalement, il propose au lancement suivant de reprendre vos modifications.
- Nouvelle rubrique Fichiers dans les réglages : intervalle de la sauvegarde automatique, ou la désactiver.
- Un document protégé par mot de passe n'est jamais copié en clair sur le disque par la sauvegarde automatique.

## 0.0.7 — 2026-11-16

- Ajout d'images (PNG, JPEG, WebP) en mode Éditer : bouton Image, coller avec Ctrl+V, ou glisser-déposer le fichier sur la page.
- Les images se déplacent et s'agrandissent par leurs coins, en gardant leurs proportions si vous le souhaitez ; elles se tournent d'un quart de tour et peuvent être rendues transparentes.
- La transparence des PNG est conservée, les photos JPEG sont incluses sans perte de qualité, et les photos prises en portrait s'affichent dans le bon sens.

## 0.0.6 — 2026-11-16

- Nouveau mode Éditer : cliquez sur la page pour ajouter du texte, puis choisissez police, taille, gras, italique, couleur et alignement.
- Les zones de texte se déplacent et s'élargissent à la souris, ou au millimètre près dans le panneau Propriétés ; elles se dupliquent et se suppriment.
- Annuler et rétablir chaque modification (Ctrl+Z, Ctrl+Y).
- Enregistrer (Ctrl+S) et Enregistrer sous (Ctrl+Maj+S) : le fichier n'est remplacé qu'une fois le nouveau contenu vérifié, et un document protégé garde son mot de passe.
- Avant de fermer un document modifié ou de quitter, PdfEditor propose d'enregistrer.

## 0.0.5 — 2026-11-16

- Mises à jour automatiques : PdfEditor recherche les nouvelles versions au démarrage puis toutes les 6 heures, les annonce avec leurs nouveautés, et ne les installe que sur votre clic, après vérification de leur signature.
- Après une mise à jour, un bandeau présente les nouveautés ; les notes de toutes les versions restent consultables dans les réglages.
- [Windows] Installeur pour Windows, moteur PDF inclus.

## 0.0.4 — 2026-11-16

- Page Réglages (menu ou Ctrl+,) : thème clair, sombre ou celui du système, zoom à l'ouverture, mémoire pour les pages, raccourcis clavier modifiables, détail du journal. Les réglages s'appliquent tout de suite.
- Documents récents sur l'accueil, avec leur dossier et la date d'ouverture ; option pour rouvrir le dernier document au démarrage.
- Journal de diagnostic, sans jamais le nom ni le contenu des documents ; bouton pour ouvrir son dossier.

## 0.0.3 — 2026-11-16

- Le texte des pages se sélectionne et se copie, ligne par ligne, y compris sur les pages tournées.
- Recherche dans tout le document (Ctrl+F) : sans tenir compte des majuscules ni des accents, résultats surlignés, suivant avec Entrée, précédent avec Maj+Entrée.

## 0.0.2 — 2026-11-16

- Première fenêtre : ouvrir un PDF (bouton, Ctrl+O, glisser-déposer ou « Ouvrir avec »), le lire en défilement continu.
- Miniatures des pages, plan du document, aller à une page par son numéro.
- Zoom : boutons, Ctrl + molette, Ctrl +/−, 100 % (Ctrl+0), page entière (Ctrl+1), largeur (Ctrl+2).
- Documents protégés par mot de passe : le mot de passe est demandé, une erreur de saisie est signalée.
- Thème clair ou sombre selon le système.

## 0.0.1 — 2026-11-16

- Fondations du projet : architecture, roadmap, intégration continue.
