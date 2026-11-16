# Brancher un outil sur paravent

Ce guide mène d'un outil qui affiche des données personnelles à un outil qu'un agent peut utiliser
sans les voir. Vocabulaire et garanties : [`cadrage.md`](cadrage.md). Commandes et codes :
[`codes.md`](codes.md).

Deux niveaux, à choisir selon ce qu'on maîtrise :

| Vous avez… | Niveau | Ce guide |
| --- | --- | --- |
| un outil existant, ou une source que vous ne développez pas | 1, outil filtré | [Niveau 1](#niveau-1--brancher-un-outil-existant) |
| une application que vous développez | 2, source aveugle | [Niveau 2](#niveau-2--rendre-une-source-aveugle) |

Au niveau 1, les données traversent l'outil sur la machine de l'agent, et seul ce que l'outil écrit
est filtré. Au niveau 2, rien ne quitte la source. Dès qu'on maîtrise la source, viser le niveau 2.

## Avant de commencer

Détail de chaque étape : [`installation.md`](installation.md).

1. Installer le binaire, depuis une copie du dépôt : `cargo install --path .`
2. Créer la clé de l'installation, lisible de vous seul :
   `(umask 077; mkdir -p ~/.config/paravent; head -c 32 /dev/urandom > ~/.config/paravent/cle)`
3. Vérifier : `paravent filtrer --declaration exemples/declaration.json < exemples/contacts.jsonl`
   rend des références et des comptes, aucun nom.

La clé se protège comme les données : qui l'a, avec la liste des identifiants, retrouve à quel
objet répond une référence. Elle ne va ni dans un dépôt ni dans une conversation.

## Niveau 1 : brancher un outil existant

### 1. Recenser ce que l'outil lit

Lister les types d'objets que l'outil manipule (contact, dossier, message…) et, pour chacun, tous
les champs qu'il peut recevoir de la source. Partir du schéma ou de la réponse de l'API, pas de ce
que l'outil affiche aujourd'hui.

### 2. Écrire la déclaration

```json
{
  "version": 1,
  "types": {
    "contact": {
      "publics": ["rôle", "état"],
      "personnels": ["prénom", "nom", "courriel"],
      "secrets": ["jeton"],
      "comptes": ["état"]
    }
  }
}
```

- `publics` : ce que l'agent peut lire tel quel.
- `personnels` : ce dont il ne connaîtra que la présence. Un champ oublié est traité comme
  personnel : dans le doute, ne pas le lister.
- `secrets` : ce que l'outil ne doit même pas lire. S'il l'envoie, le filtre s'arrête.
- `comptes` : les champs publics dont on veut le décompte par valeur.

Un nom de type s'écrit en minuscules, chiffres et `_`, sans tiret.

### 3. Faire relire la déclaration par un humain

C'est l'étape qui porte la sûreté : un champ classé public à tort sort tel quel. La relecture suit
la [liste de contrôle](#liste-de-contrôle-de-relecture-dune-déclaration). Un agent ne valide pas
une déclaration, et ne la modifie pas pour faire passer une commande.

### 4. Faire passer toutes les sorties par le filtre

**Outil en Python.** Copier [`python/paravent.py`](../python/paravent.py) à côté de l'outil et
mettre tout son travail dans un bloc :

```python
import paravent

with paravent.Sortie(declaration="declaration.json") as sortie:
    for contact in lire_les_contacts():
        sortie.emettre("contact", contact["id"], contact)
    sortie.etat("LISTE_TERMINEE", total=3)
```

Dans le bloc, `print` est refusé et une exception ne laisse sortir que son type et sa ligne. Pour
dire autre chose que des objets, `sortie.etat` prend un code et des comptes entiers. Outil
complet : [`exemples/outil_python/`](../exemples/outil_python/).

**Autre langage.** Écrire un objet JSON par ligne et le passer au binaire :

```sh
outil lister-en-clair | paravent filtrer --declaration declaration.json
```

L'agent ne doit alors pouvoir lancer que la commande entière, jamais sa moitié gauche. La sortie
d'erreur et les fichiers de l'outil restent à sa charge : le banc les vérifie.

### 5. Préparer le jeu d'essai et le plan du banc

Dans une copie fictive des données, placer une valeur piège reconnaissable dans chaque champ
personnel : `PIEGE-prenom-7f3a`, `piege.courriel@exemple.invalid`. Puis écrire le plan :

```json
{
  "pieges": ["PIEGE-prenom-7f3a", "piege.courriel@exemple.invalid"],
  "commandes": [
    ["outil", "lister"],
    ["outil", "chercher", "PIEGE-prenom-7f3a"],
    ["outil", "commande-qui-plante"]
  ],
  "surveiller": ["sorties", "outil.log"]
}
```

Le plan liste **toutes** les commandes de l'outil, y compris celles qui échouent : une trace
d'erreur est une fuite fréquente. `surveiller` liste les fichiers et dossiers où l'outil écrit,
journal compris. Les données fictives, la clé et le plan ne servent qu'au banc.

### 6. Obtenir un banc vert

```sh
paravent banc --plan plan.json
```

`BANC_OK` : l'outil peut être confié à un agent. `FUITE …` donne la commande, l'emplacement et le
rang du piège ; corriger l'outil et rejouer. Garder le plan dans le dépôt de l'outil et rejouer le
banc à chaque changement de l'outil ou de la déclaration, en CI si le dépôt en a une.

### 7. Dire aux agents de passer par là

Inscrire la [règle](#règle-à-inscrire-dans-les-consignes-des-agents) dans les consignes du dépôt de
l'outil, et retirer aux agents les chemins qui contournent l'outil : accès direct à la base,
export en clair, jeton d'API lisible.

## Niveau 2 : rendre une source aveugle

Ici la source elle-même ne répond qu'en forme sûre. En Rust, elle importe la bibliothèque ; dans un
autre langage, elle applique les mêmes règles et se prouve par le banc. Exemple complet, qui
s'exécute : `cargo run --example source_aveugle`
([`exemples/source_aveugle.rs`](../exemples/source_aveugle.rs)).

### 1. Déclarer, et faire relire

Comme au niveau 1, étapes 1 à 3. La déclaration vit dans le dépôt de la source.

### 2. Donner à l'agent une interface à part

Des routes ou des commandes réservées aux agents, qui ne renvoient que des formes sûres. Les routes
de l'interface humaine, qui renvoient les données en clair, ne sont pas accessibles avec les droits
d'un agent.

### 3. Répondre en forme sûre

```rust
let mut filtre = Filtre::new(declaration.clone(), Fabrique::new(cle.clone()));
let sur = filtre.traiter(objet)?;      // référence, champs publics, présences
repondre(sur.en_json());               // puis filtre.comptes().en_json()
```

Les messages d'erreur de la source suivent la même règle : un code et une position, jamais la
valeur fautive.

### 4. Offrir la résolution

L'humain dit « écris à Camille, de l'école des Tilleuls ». L'agent transmet ces termes ; la source
cherche en clair et rend des références. S'il y a plusieurs réponses, l'agent les départage par
leurs champs publics ou demande à l'humain. La source fait le chemin inverse, de la référence à
l'objet, en recalculant ou en stockant ses références.

### 5. Faire attendre toute action qui vise une personne

```rust
let controle = Modele::analyser(texte)?.controler(&declaration, cible.type_objet())?;
let action = Action::proposer(&id, "courriel", cible, &controle, auteur, maintenant)?;
```

La source garde l'action. Rien ne part. Contrat : [ADR 0002](adr/0002-actions-en-attente.md).

### 6. Construire l'écran de validation

Dans l'interface humaine, la personne voit le destinataire et le message rempli, en clair, puis
valide ou refuse. C'est la seule entrée de `Action::valider`, et `Humain::atteste` ne s'appelle que
depuis la session d'une personne authentifiée, jamais avec les droits d'un agent.

### 7. Exécuter après validation, et tenir un journal aveugle

Au moment d'exécuter une action validée, la source remplit le modèle (`ModeleControle::remplir`),
agit, puis appelle `marquer_executee`. Le journal ne porte que des références, des comptes et des
états.

### 8. Passer le banc

Écrire le plan avec les commandes ou les appels que l'agent utilisera, sur une base fictive semée
de pièges, et obtenir `BANC_OK`. Surveiller aussi le journal de la source.

## Liste de contrôle de relecture d'une déclaration

Un humain qui connaît les données coche chaque ligne, pour chaque type.

- [ ] La liste des champs vient du schéma de la source, pas de la sortie actuelle de l'outil.
- [ ] Chaque champ public a été relu sur des valeurs réelles : aucune ne nomme ni ne décrit une
      personne.
- [ ] Aucun champ public n'est un texte libre (commentaire, objet d'un message, remarque).
- [ ] Aucun champ public n'est un identifiant d'origine, une adresse, un numéro ou une date de
      naissance.
- [ ] Aucun champ public ne désigne quelqu'un par recoupement : une fonction tenue par une seule
      personne, un lieu où il n'y a qu'un contact.
- [ ] Les noms d'organisations ou de lieux déclarés publics le sont par décision, pas par défaut.
- [ ] Mots de passe, jetons, clés et liens d'accès sont en `secrets`.
- [ ] Les champs de `comptes` ne produisent pas de très petits nombres qui désignent quelqu'un.
- [ ] Un champ dont on doute n'est pas listé : il reste personnel.
- [ ] La relecture est datée et signée dans le dépôt de l'outil, et refaite à chaque changement du
      schéma.

## Règle à inscrire dans les consignes des agents

À adapter, puis à placer dans les consignes que lisent vos agents.

> **Données personnelles : passer par paravent.** Un outil qui lit des données où figurent des
> personnes ne s'utilise que branché sur `paravent`, avec un banc anti-fuite vert. Un agent ne lit
> ni la base, ni un export, ni la clé. Il travaille avec des références, des comptes et des états.
> Pour agir sur une personne, il propose une action avec un modèle à trous ; un humain la valide
> dans l'application. Sur `TYPE_INCONNU`, `CHAMP_SECRET`, `COLLISION` ou `FUITE`, il s'arrête et le
> signale ; il ne modifie pas une déclaration. Avant de créer ou de modifier un tel outil, il lit
> le guide de branchement.

## Ce que paravent ne couvre pas

- Ce qui ne passe pas par un outil branché : un fichier lisible sur le poste, un accès direct à la
  base, un envoi par le réseau.
- Ce que l'humain tape lui-même dans la conversation.
- Une déclaration mal classée, et les très petits nombres.

Détail : « Limites connues » du [cadrage](cadrage.md).
