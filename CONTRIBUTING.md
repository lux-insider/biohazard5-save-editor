# Como contribuir

Obrigado pelo interesse! Este é um projeto pessoal e gratuito, mantido no
tempo livre. Toda ajuda é bem-vinda: relatar erros, sugerir ideias ou enviar
código.

## Relatar erros e sugerir ideias

Use as [issues](https://github.com/lux-insider/biohazard5-save-editor/issues/new/choose): há um
modelo para erros e outro para ideias. Para falhas de segurança, veja o
[SECURITY.md](SECURITY.md).

## Compilar e testar

Precisa do [Rust](https://rustup.rs). Os passos completos, inclusive a
compilação para Windows a partir do Linux, estão na seção
[Compilação](README.md#compilação) do README.

```
cargo build --release
cargo test --release
```

## Enviar uma mudança

1. Crie um branch a partir da `main`.
2. Faça a mudança com testes, quando der.
3. Atualize o [CHANGELOG.md](CHANGELOG.md) se a mudança aparece para quem usa.
4. Abra o pull request e preencha o modelo.

## Estilo

- Textos do programa, comentários e documentação em **português**.
- **Não rode `cargo fmt`**: o código deste projeto não segue esse formato e o diff ficaria enorme.
- Commits curtos e no imperativo, em português (ex.: "Corrige o checksum do Xbox").

- Nunca inclua `kv.bin`, saves de outras pessoas ou dados de perfil reais. Os prints usam saves com IDs fictícios.

## Licença

Ao contribuir, você concorda que o seu código seja distribuído sob a
[licença MIT](LICENSE) do projeto.
