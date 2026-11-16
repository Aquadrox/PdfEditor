# Design

Direction visuelle et règles d'interface. Toute nouvelle page ou tout nouveau
composant s'y conforme ; une exception se discute dans la PR et, si elle est
retenue, s'ajoute ici.

## Intention

**Sujet** : des documents qu'on lit, corrige, traduit et signe — contrats,
devis, formulaires, courriers. **Public** : indépendants, petites équipes,
particuliers exigeants ; utilisation sur PC, souvent plusieurs heures.
**Travail principal de l'interface** : montrer le document, fidèlement, et
s'effacer derrière lui.

Le vocabulaire visuel vient du bureau d'un juriste : le **papier** posé sur
une **table lumineuse** grise, l'**encre bleue** du stylo qui annote et
sélectionne, la **cire bordeaux** du sceau qui signe et verrouille. Pas de
dégradés, pas de cartes décoratives : la page est le seul objet en relief.

**L'élément mémorable** (et le seul audacieux) : **le sceau**. L'état de
signature du document — non signé, signé, verrouillé, cycle en cours — est
porté par une pastille de cire dans la barre du haut et par un liseré
bordeaux le long des pages verrouillées. Quand l'utilisateur signe, le sceau
se « presse » une fois (seule animation non triviale de l'application). Tout
le reste reste calme et discipliné.

## Couleurs

Jetons définis une seule fois dans `ui/styles/tokens.css` ; aucune couleur en
dur ailleurs. Contrastes vérifiés (WCAG 2.2 AA, 4,5:1 minimum pour le texte)
sur le fond des panneaux et sur la table.

| Jeton | Rôle | Clair | Sombre |
|---|---|---|---|
| `--desk` | Table lumineuse autour des pages | `#E3E6EA` | `#181B1F` |
| `--surface` | Panneaux, barres | `#F6F7F8` | `#22262B` |
| `--paper` | Page (ne change jamais avec le thème) | `#FFFFFF` | `#FFFFFF` |
| `--ink` | Texte principal | `#1C2430` | `#E6E9ED` |
| `--muted` | Texte secondaire, icônes au repos | `#566170` | `#9AA4AF` |
| `--line` | Bordures, séparateurs | `#D5DAE0` | `#343A41` |
| `--accent` | Encre bleue : action principale, sélection, focus | `#2747B8` | `#8FA6FF` |
| `--seal` | Cire : signature, verrouillage, cycle | `#8A1C2B` | `#F0909C` |
| `--ai` | Propositions de l'IA (contour, marge, différences) | `#0F6E67` | `#5CCFC3` |
| `--ok` | Signature valide, enregistré | `#1E7344` | `#5CC98D` |
| `--warn` | Signature à vérifier, débordement | `#8A5700` | `#E8B65A` |
| `--danger` | Signature invalide, caviardage, suppression | `#B3261E` | `#F28B82` |

Règles :

- La couleur porte un **sens**, jamais une décoration : bleu = ce que je fais,
  bordeaux = ce qui engage (signature), sarcelle = ce que propose l'IA.
- La couleur n'est jamais le seul signal : chaque état a aussi une icône et un libellé.
- La page reste blanche en thème sombre (fidélité du document) ; seule la table s'assombrit.

## Typographie

| Usage | Police | Raison |
|---|---|---|
| Interface | **IBM Plex Sans** (OFL, embarquée) | Dessin net à petite taille, chiffres tabulaires pour coordonnées, zoom et numéros de page ; identique sous Windows et Linux |
| Signature tapée | **Caveat**, **Homemade Apple**, **Dancing Script** (OFL / Apache, embarquées) | Au choix dans l'apparence de signature |
| Texte ajouté au PDF | Polices standard du PDF + **Noto Sans / Noto Serif** (OFL) embarquées | Couverture des alphabets pour l'édition et la traduction |

Échelle (interface dense de bureau) : `12` légendes · `13` texte de base ·
`15` titres de panneau · `18` titres de page des Réglages · `22` titres de
dialogue. Graisses 400 et 600 seulement. Interligne 1,45. Lignes de texte
longues (aide, résultats de l'IA) limitées à 72 caractères.

Interdits : capitales pour les libellés, mot isolé mis en couleur dans un
titre, sur-titres décoratifs au-dessus des sections.

## Espacements, formes, relief

- Grille de **4 px** : `--space-1` 4 · `-2` 8 · `-3` 12 · `-4` 16 · `-6` 24 · `-8` 32.
- Rayons hiérarchisés : contrôles `4 px`, panneaux flottants et menus `6 px`,
  **pages `0`** (le papier a des coins vifs).
- Relief : seule la page a une ombre (papier posé sur la table) ; les panneaux
  sont plats, séparés par des bordures `--line` ; menus et dialogues ont une
  ombre courte.
- Densité : hauteur de contrôle 28 px (cible minimale 24 px, WCAG 2.2) ; mode
  « confortable » 32 px dans les réglages.

## Disposition de l'éditeur

```
┌───────────────────────────────────────────────────────────────────────────┐
│ ≡  contrat-bail.pdf •      Lire  Éditer  Pages  Signer  IA      [◉ sceau] │ barre du haut
├───────────────────────────────────────────────────────────────────────────┤
│ T Texte  ▣ Image  ✎ Dessin  ▭ Forme  ▤ Caviarder  │ Police 11 pt  ■  ⟲ ⟳ │ outils du mode
├────────┬─────────────────────────────────────────────────┬────────────────┤
│ ▢ 1    │                                                 │ Propriétés     │
│ ▢ 2    │        ┌─────────────────────────┐              │ (sélection)    │
│ ▣ 3    │        │                         │              │                │
│ ▢ 4    │        │      page (papier)      │   table      │ ou Signatures  │
│        │        │                         │   lumineuse  │ ou Assistant IA│
│ Plan   │        └─────────────────────────┘              │                │
├────────┴─────────────────────────────────────────────────┴────────────────┤
│ Page 3 / 12   ·   110 %   ·   Enregistré   ·   2 signatures valides       │ barre d'état
└───────────────────────────────────────────────────────────────────────────┘
```

- **Modes** (Lire, Éditer, Pages, Signer, IA) dans la barre du haut : chacun
  n'affiche que ses outils ; on sait toujours dans quel mode on est.
- Panneau gauche : miniatures / plan, repliable. Panneau droit : contextuel,
  repliable, redimensionnable. Les deux se ferment pour lire (`F11` plein écran).
- Contenu aligné à gauche partout ; seules les pages sont centrées sur la table.
- Fenêtre minimale 960 × 600 ; en dessous de 1200 px de large, le panneau
  droit passe en tiroir par-dessus la table.

## Composants

Bibliothèque unique dans `ui/components/` (un fichier JS + un fichier CSS par
composant), chacun avec tous ses états : repos, survol, appuyé, désactivé,
focus clavier, chargement, erreur.

Bouton (principal, secondaire, discret, danger) · champ texte / nombre avec
unité · liste déroulante · interrupteur · curseur · sélecteur de couleur ·
menu · info-bulle (avec raccourci) · dialogue · tiroir · onglets · arbre (plan)
· miniature de page · notification (non bloquante, avec « Annuler » quand c'est
possible) · pastille d'état (sceau) · vue de différences (IA).

Icônes : **Lucide** (licence ISC, comme VoiceToText), trait 1,75, 18 px ;
une icône = un sens, le même partout.

## Propositions de l'IA

- Une proposition n'est jamais appliquée directement : elle s'affiche en
  **marge sarcelle** à côté du bloc, texte d'origine barré, texte proposé
  souligné, avec « Accepter », « Refuser », « Tout accepter ».
- Ce qui vient de l'IA est toujours identifié (icône et couleur `--ai`),
  jusqu'à son acceptation ; ensuite c'est une modification ordinaire, annulable.
- Avant un envoi : portée, fournisseur, volume et coût estimés, visibles en
  une ligne au-dessus du bouton d'envoi.

## Mouvement

- Uniquement en réponse à une action : ouverture de panneau, apparition d'un
  menu, sélection, confirmation. Durées 120 à 180 ms, `ease-out`.
- Une seule séquence orchestrée : le sceau qui se presse à la signature.
- `prefers-reduced-motion` : toutes les transitions deviennent instantanées.
- Pas de chargement décoratif : une barre de progression déterminée quand la
  durée est connue (rendu, enregistrement, IA par lots), sinon un indicateur discret.

## Accessibilité

- Tout est faisable au clavier ; ordre de tabulation logique ; focus visible
  (anneau `--accent` 2 px) ; raccourcis listés et modifiables (R-08).
- Rôles et libellés ARIA sur les outils et panneaux ; la couche texte des
  pages est lisible par les lecteurs d'écran.
- Zoom de l'interface (Ctrl + / −) indépendant du zoom du document.
- Contrastes AA vérifiés pour chaque nouveau jeton.

## Rédaction

- Français, phrases courtes, casse de phrase, voix active.
- Un bouton dit ce qu'il fait, et l'action garde son nom tout au long :
  « Signer et verrouiller » → notification « Document signé et verrouillé ».
- Les erreurs disent ce qui s'est passé et comment corriger, sans s'excuser :
  « Mot de passe incorrect. Vérifiez les majuscules. »
- Un écran vide invite à agir : « Ouvrez un PDF ou déposez-le ici ».
- Les termes techniques (PAdES, DocMDP, RFC 3161) restent dans les détails
  avancés ; l'interface dit « signature », « verrouiller », « horodatage ».

## Organisation du code de l'interface

```
apps/desktop/ui/
├── index.html
├── styles/      tokens.css · base.css · layout.css (un fichier par zone)
├── components/  button.js/.css · dialog.js/.css · …
├── features/    viewer/ · edit/ · pages/ · sign/ · cycle/ · ai/ · settings/
└── lib/         ipc.js (seul fichier qui appelle Tauri) · store.js · i18n.js
```

Les textes de l'interface passent par `i18n.js` (français d'abord, anglais
prévu) ; aucun contenu de document n'est inséré autrement qu'avec `textContent`.

## Références

Principes recoupés avec les guides fournis par le propriétaire (skills de
design pour Claude : « Frontend Design », « Impeccable » `/critique` et
`/audit`) : jetons centralisés, échelle typographique courte, un seul jeu
d'icônes, tous les états de composant, contraste AA, animations brèves et
réservées aux actions. Une revue de design (captures d'écran, critique,
audit d'accessibilité) est faite à chaque version qui ajoute des écrans.
