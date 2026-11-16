---
name: paravent
description: Work on data that contains people without seeing it - use, create or modify a CLI or source that reads personal data (names, emails, phones, free text) through the paravent filter, declare its fields, prove it with the leak bench, or prepare an action that targets a person.
---

# paravent : agir sans voir

`paravent` est le filtre entre un agent et des données personnelles. Tu travailles **aveugle** :
tu reçois des références (`contact-f8z00rja`), des champs publics, des présences
(`courriel:renseigné`), des comptes et des états. Les valeurs restent du côté de la source.

Dépôt : celui qui contient cette skill. Guide complet : `docs/guide-branchement.md`. Commandes et
codes : `docs/codes.md`.

## Utiliser un outil branché

- Désigne un objet par sa référence. Pour partir de ce que l'humain a dit (« Camille, de l'école
  des Tilleuls »), demande la résolution à l'outil : il rend des références.
- Plusieurs réponses : départage par les champs publics, sinon demande à l'humain. Choisis sur des
  faits, jamais au hasard.
- Pour agir sur une personne, propose une action avec un modèle à trous :
  `Bonjour {prénom}, …`. Contrôle-le d'abord : `paravent modele --declaration <fichier> --type <type>`.
  L'action attend ; un humain la valide dans l'application qui détient les données, pas dans
  `paravent` : dis-lui qu'elle l'attend, sans inventer un écran que tu n'as pas vu.
- Copie une référence depuis la sortie de l'outil, caractère pour caractère. Ne la retape pas.
- Travaille avec ce que l'outil te donne. La base, les exports, la clé et les routes de l'interface
  humaine sont hors de ton périmètre, même si tu y as techniquement accès.

## Quand ça s'arrête

| Code | Ce que tu fais |
| --- | --- |
| `TYPE_INCONNU`, `CHAMP_SECRET`, `COLLISION`, `FUITE` | Tu t'arrêtes et tu le signales à l'humain. La déclaration se corrige par lui. |
| `MODELE_INVALIDE` | Tu corriges ton modèle : le message nomme le champ ou la position. |
| `LIGNE_ILLISIBLE`, `ECRITURE_DIRECTE`, `EXCEPTION` | L'outil a un défaut : tu le signales, ou tu le corriges si c'est ta tâche. |
| `CLE_REFUSEE`, `BINAIRE_ABSENT`, `BINAIRE_TROP_ANCIEN` | L'installation est incomplète : tu le signales. |

## Créer ou modifier un outil qui lit des personnes

Suis `docs/guide-branchement.md`. L'essentiel :

1. Recense les types et les champs depuis le schéma de la source.
2. Écris la déclaration. Un champ dont tu doutes reste hors de `publics`.
3. Fais-la relire par un humain, avec la liste de contrôle du guide. Elle n'est valable qu'après.
4. Fais passer toute sortie par le filtre : `paravent.Sortie` en Python, `paravent filtrer` sinon.
5. Travaille sur des données fictives, semées de valeurs pièges. Écris le plan du banc avec
   **toutes** les commandes, y compris celles qui échouent.
6. Livre quand `paravent banc --plan plan.json` répond `BANC_OK`, et cite cette sortie.

## Ce que tu dis à l'humain

Ce qu'il tape dans la conversation passe par toi : un prénom dicté n'est pas masqué. S'il veut
l'éviter, il te donne une référence.
