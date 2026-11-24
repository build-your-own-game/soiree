# Le relais de la console (`relais/`), en un binaire statique dans une image
# vide. C'est la seule partie de ce dépôt qui tourne sur un serveur : les jeux
# tournent sur l'ordinateur de l'hôte.
FROM rust:1-alpine AS construction
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
# Le compilateur de l'image suffit : sans ce fichier, rien n'est téléchargé
# pour les cibles des jeux (WebAssembly), dont le relais n'a pas besoin.
RUN rm rust-toolchain.toml && cargo build --release --locked --package relais

FROM scratch
COPY --from=construction /src/target/release/relais /relais
USER 65534:65534
EXPOSE 8080 8081
ENTRYPOINT ["/relais"]
