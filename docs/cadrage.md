# Cadrage de paravent

État : livré en version 0.1.0. Ce document fixe le vocabulaire, les garanties et les limites.

## Le problème

Un agent qui pilote un outil en ligne de commande lit tout ce que l'outil affiche. Si l'outil
interroge une base où vivent des personnes, leurs données partent chez le fournisseur du modèle.
Réserver ces tâches à des modèles locaux ou sans conservation règle la fuite, mais prive des
meilleurs modèles et repose sur un réglage qu'on peut oublier.

`paravent` vise l'autre voie : **un seul mode, aveugle**. L'agent garde sa capacité d'agir ; les
valeurs personnelles ne lui parviennent jamais.

Le principe est connu : le modèle manipule des références, et c'est un programme déterministe qui
les remplace par les valeurs au moment d'agir (« Dual LLM », puis CaMeL). `paravent` l'applique à
des données structurées, où l'on sait d'avance quel champ est personnel : rien n'est à détecter.

## Vocabulaire

| Terme | Sens |
| --- | --- |
| **Source** | Le système qui détient les données : une base, une API, une boîte de courriel. |
| **Outil** | La commande que l'agent lance pour interroger ou modifier une source. |
| **Déclaration** | Le fichier où un outil classe chaque champ de chaque type d'objet. |
| **Champ public** | Peut sortir tel quel : un état, une date, un nom d'établissement. |
| **Champ personnel** | Ne sort jamais : nom, courriel, téléphone, texte libre écrit par une personne. |
| **Champ secret** | Ne sort jamais et n'est jamais lu par l'outil : mot de passe, jeton. |
| **Référence** | Un identifiant opaque et stable qui désigne un objet : `contact-4b1e9c2a`. |
| **Résolution** | Passer de termes donnés par l'humain (« Sophie », « Tilleuls ») à des références. |
| **Modèle à trous** | Un texte dont les valeurs personnelles sont des champs nommés : `{prénom}`. |
| **Action en attente** | Une action préparée par l'agent, que seul un humain peut déclencher. |
| **Banc anti-fuite** | Le test qui joue toutes les commandes d'un outil et échoue si une valeur piège sort. |

## Garanties

Chacune a un test qui échoue quand elle est rompue.

1. **Refus par défaut.** Un champ absent de la déclaration est traité comme personnel.
2. **Sortie unique.** Tout ce qu'un outil écrit (sortie standard, sortie d'erreur, fichiers de
   résultat, journal) passe par le filtre. Les messages d'erreur d'une source y passent aussi.
3. **Références stables et opaques.** Le même objet donne toujours la même référence ; une
   référence ne révèle rien et ne s'inverse pas sans la clé locale.
4. **Remplissage hors de l'agent.** Un modèle à trous est rempli du côté de la source, jamais dans
   le processus que lit l'agent.
5. **Rien d'individuel sans humain.** Une action qui vise une personne reste en attente jusqu'à
   validation, là où l'humain voit les données en clair.
6. **Journal aveugle.** Le journal ne contient que des références, des comptes et des états.

## Ce que l'agent voit

- Des références, des comptes, des états, les champs publics.
- Pour un champ personnel : sa présence (« courriel : renseigné »), jamais sa valeur.
- Ce que l'humain tape lui-même dans la conversation. Un prénom dicté à l'agent transite par le
  modèle : c'est la limite assumée du mode aveugle. Qui veut l'éviter donne une référence.

## Deux niveaux de branchement

| Niveau | Où agit le filtre | Pour qui | Ce qui reste exposé |
| --- | --- | --- | --- |
| **1. Outil filtré** | L'outil reçoit les données en clair et les passe au filtre avant d'écrire | Les outils existants, les sources qu'on ne maîtrise pas | Les données sont dans la mémoire du processus, sur la machine de l'agent |
| **2. Source aveugle** | La source elle-même ne renvoie que des références et des champs publics | Les applications qu'on développe | Rien ne quitte la source |

Le niveau 2 est la cible dès qu'on maîtrise la source. Le niveau 1 est le chemin de mise à niveau
des outils existants.

## Livrables

| Livrable | Forme | Rôle |
| --- | --- | --- |
| Bibliothèque | Crate Rust `paravent` | Déclaration, références, filtrage, modèles à trous. Importée par les sources écrites en Rust (niveau 2). |
| Filtre | Binaire `paravent` | Lit des objets structurés sur son entrée, écrit la sortie sûre. Sert tout outil, quel que soit son langage (niveau 1). |
| Adaptateur Python | Un fichier, bibliothèque standard seule | Envoie la sortie d'un outil Python au binaire et interdit l'écriture directe. |
| Banc anti-fuite | Commande `paravent banc` | Sème des valeurs pièges, joue les commandes déclarées, cherche les pièges partout où l'outil écrit. |

### Format d'échange

Un outil envoie au filtre des objets, un par ligne :

```json
{"type": "contact", "id": "1042", "champs": {"prénom": "…", "courriel": "…", "rôle": "enseignante"}}
```

Le filtre remplace `id` par une référence, laisse passer les champs publics, réduit les champs
personnels à leur présence, et rejette un objet dont le type n'est pas déclaré.

### Références

`<type>-<8 caractères>`, tirés d'un HMAC-SHA-256 de `type` et `id` sous une clé propre à chaque
installation. La clé vit hors dépôt, lisible du seul propriétaire. Changer de clé change toutes
les références : c'est un moyen d'oubli, pas un incident.

Une source de niveau 2 peut préférer des références qu'elle attribue et stocke elle-même ; la
bibliothèque accepte les deux.

### Dépendances

Rust n'a ni JSON ni SHA-256 dans sa bibliothèque standard. Dépendances proposées, et elles seules :
`serde`, `serde_json`, `hmac`, `sha2`. Toute autre passe par un ticket.

## Limites connues

- **Pas un coffre.** `paravent` garantit ce qui sort des outils branchés. Un agent qui a un terminal
  peut lire un fichier lisible : les fichiers sensibles se protègent par les droits du poste.
- **Petits nombres.** « Un seul contact sans courriel dans cette école » désigne quelqu'un. La
  première version ne traite pas ce cas ; elle le documente.
- **Texte libre.** Un commentaire écrit par une personne est personnel en entier. L'agent n'en
  connaît que la présence.
- **Champs publics mal classés.** La sûreté dépend de la déclaration. Sa relecture par un humain
  fait partie de la mise en service d'un outil.

## Publication

- Dépôt public : <https://github.com/Bricesodini/paravent>, sous licence MIT.
- Conséquence : aucun nom de personne, d'organisation, d'hôte ou de projet privé dans le dépôt, son
  historique, ses tickets et ses exemples. Les exemples sont inventés.

## Ordre de construction

1. Bibliothèque : déclaration, refus par défaut, références.
2. Binaire filtre et format d'échange.
3. Banc anti-fuite.
4. Adaptateur Python.
5. Modèles à trous et contrat des actions en attente.
6. Premier outil branché, sur une application réelle, pour éprouver les deux niveaux.
7. Guide « brancher un outil existant », puis un ticket par outil à mettre à niveau.

Les étapes 1 à 4 ne dépendent d'aucune application. L'étape 6 se mène dans le dépôt de
l'application, qui garde ses règles métier.
