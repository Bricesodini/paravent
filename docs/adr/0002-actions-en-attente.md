# ADR 0002 — Modèles à trous et actions en attente

État : appliqué en version 0.1.0.

## Contexte

L'agent doit pouvoir demander « écris à cette personne » sans voir ni son nom ni son adresse
(garanties 4 et 5 du cadrage). Deux choses lui manquent donc : un moyen d'écrire un texte sans les
valeurs, et un moyen de demander une action sans pouvoir la déclencher.

## Décision

### Modèle à trous

- Un trou s'écrit `{champ}`. Une accolade littérale se double : `{{`, `}}`.
- Le contrôle se fait contre la déclaration, pour un type d'objet. Sont refusés : un type non
  déclaré, un champ secret, un champ non déclaré. Ce dernier refus va à l'inverse du filtre, où un
  champ non déclaré passe pour personnel : ici, une faute de frappe partirait dans un message.
- Le contrôle rend la liste des champs utilisés. La source sait ainsi ce qu'elle doit lire.
- Le remplissage est une fonction de la bibliothèque. Son résultat est une valeur personnelle, qui
  ne s'affiche pas. Le binaire ne remplit rien : `paravent modele` contrôle seulement.
- Une valeur n'est jamais relue comme un modèle. Une valeur absente ou vide arrête le remplissage :
  « Bonjour , » ne part pas.

### Action en attente

Une action est ce que l'agent dépose et que la source garde :

| Champ | Contenu |
| --- | --- |
| `id` | Identifiant attribué par la source. |
| `nature` | Ce que la source fera : `courriel`, `rappel`… |
| `cible` | La référence de l'objet visé. |
| `modele` | Le texte du modèle, non rempli. |
| `champs` | Les champs que le modèle utilise. |
| `auteur` | Le nom de l'agent qui propose. |
| `creee_le` | Date de la proposition. |
| `etat` | `en_attente`, `validee`, `refusee`, `executee`. |
| `decision` | Qui a décidé et quand : `{"par", "le"}`. Absent tant que l'action attend. |
| `executee_le` | Date d'exécution. |

Les passages permis, et eux seuls :

```
en_attente ── valider (humain, date) ──▶ validee ── marquer exécutée (date) ──▶ executee
     └──────── refuser (humain, date) ──▶ refusee
```

- Aucune fonction ne mène à `executee` sans passer par `validee`, et `validee` exige un humain et
  une date.
- L'auteur d'une action ne peut pas la décider.
- La relecture depuis du JSON applique les mêmes règles : une action « exécutée » sans décision,
  ou décidée par son auteur, est refusée.
- Le modèle et la cible sont du même type.
- Une action ne contient aucune valeur personnelle. Le résumé destiné à l'agent ne nomme pas la
  personne qui a décidé.

## Ce que la bibliothèque ne garantit pas

- **Qu'un humain est un humain.** `Humain::atteste` se construit à partir d'un identifiant. C'est à
  la source de ne l'appeler que depuis une session où une personne s'est authentifiée. Le nom de la
  fonction rend ces appels faciles à retrouver à la relecture.
- **Que l'action exécutée est celle qui a été validée.** La source garde l'action entre la
  validation et l'exécution ; elle doit la protéger d'une modification.
- **Que l'humain a vu le bon texte.** L'écran de validation montre le message rempli et son
  destinataire en clair : il appartient à la source.
- **L'ordre des dates.** Les dates viennent de la source ; leur forme est vérifiée, pas leur ordre.

## Options écartées

- Remplir dans le binaire : le texte rempli passerait par le processus que lit l'agent.
- Laisser passer un champ non déclaré dans un modèle : une faute de frappe deviendrait un message.
- Une validation portée par un simple booléen : rien n'attribuerait la décision, et un agent
  pourrait la poser.
