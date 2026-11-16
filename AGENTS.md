# paravent — règles pour les agents

- Tout changement passe par une branche et une PR ; jamais de commit direct sur `main`.
- Lire `docs/cadrage.md` avant tout changement : il fixe le vocabulaire et les garanties.
- Ce dépôt est public. Aucun nom de personne, d'organisation, d'hôte ou
  de projet privé ; aucune donnée réelle, même en exemple. Les exemples sont inventés.
- Tests d'abord. Une garantie n'existe qu'avec le test qui échoue quand elle est rompue ; le banc
  anti-fuite fait foi.
- Dépendances Rust : celles que liste `docs/cadrage.md`, et elles seules ; toute autre passe par un
  ticket. Côté Python : bibliothèque standard seule.
- Vérifications : `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`,
  puis `python3 -m unittest discover -s python` (après `cargo build` ; demande `python3`).
- Un changement de garantie ou de format de référence passe par un ADR dans `docs/adr/`.
