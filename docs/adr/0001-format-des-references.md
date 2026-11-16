# ADR 0001 — Format des références

État : appliqué en version 0.1.0.

## Contexte

L'agent désigne un objet par une référence, jamais par son identifiant d'origine (garantie 3 du
cadrage). La référence doit être stable, opaque, courte, et pouvoir se lire à voix haute.

## Décision

- Forme : `<type>-<suffixe>`. Le type suit les règles des noms de type : lettres minuscules,
  chiffres et `_`. Il ne contient donc pas de tiret, et le premier tiret sépare toujours les deux
  parties.
- Suffixe calculé : 8 caractères, soit les 40 premiers bits d'un HMAC-SHA-256, écrits en base 32.
  Alphabet : `0123456789abcdefghjkmnpqrstvwxyz` (sans `i`, `l`, `o`, `u`, qui se confondent).
- Entrée du HMAC : la longueur du type sur 8 octets, puis le type, puis l'identifiant. La longueur
  empêche que (`ab`, `c`) et (`a`, `bc`) donnent la même empreinte.
- Clé : au moins 32 octets, dans un fichier hors dépôt, refusé s'il est accessible à d'autres que
  son propriétaire. Pour la créer :
  `(umask 077; head -c 32 /dev/urandom > ~/.config/paravent/cle)`.
- Référence fournie par la source (niveau 2) : même forme, suffixe de 4 à 64 lettres minuscules ou
  chiffres. La bibliothèque vérifie la forme ; l'opacité est à la charge de la source.

## Conséquences

- **Collisions.** Avec 40 bits, deux objets d'un même type ont environ une chance sur trois de
  se rencontrer vers un million d'objets, et une sur mille vers cinquante mille. La fabrique
  signale toute collision entre objets qu'elle voit passer dans un même processus, et s'arrête. Elle
  ne voit pas une collision entre deux exécutions : une source qui porte de grands volumes attribue
  et stocke ses propres références.
- **Changer de clé** change toutes les références. C'est le moyen d'oublier ; ce n'est pas un
  incident. Les références notées par un humain ou un agent deviennent alors inutilisables.
- **Petits ensembles.** Qui a la clé et la liste des identifiants retrouve à quel objet répond une
  référence. La clé se protège comme les données.
- **Pas de réciproque.** Une référence ne se retourne pas en identifiant par calcul : la
  résolution passe par la source, qui recalcule ou qui stocke.

## Options écartées

- Hexadécimal sur 8 caractères : 32 bits seulement, collisions probables dès soixante mille objets.
- Suffixe plus long : plus sûr, mais pénible à lire et à dicter. Une source qui en a besoin fournit
  ses propres références.
- Identifiant chiffré réversible : la référence deviendrait un secret à part entière.
