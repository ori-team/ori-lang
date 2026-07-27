# Primeiro projeto e pacotes locais

> Status: guia prático Ori **S3 / 0.3.2**  
> **English:** [first-project.md](first-project.md)  
> Layout: raiz-first (`ori.proj` + `main.orl`) — [spec/17](../spec/17-project-and-docs.md)

## Criar um projeto

```bash
ori new demo
cd demo
ori check main.orl
ori run main.orl
```

`ori new` cria:

```text
demo/
  ori.proj    # entry = "main.orl"
  main.orl
```

**Não** há pasta `src/` obrigatória. `docs/` é opcional (sidecars `.oridoc`).

## Comandos principais

```bash
ori check main.orl
ori run main.orl
ori compile main.orl --out demo
ori test main.orl
ori doctor
```

Teste:

```ori
module demo.main

import ori.test = test

@test
math_is_stable()
    test.assert(1 + 1 == 2, "math should work")
end
```

## Biblioteca local

Estrutura e manifests: veja a versão em inglês (mesmos exemplos S3 com
`module`, `import path = alias`, `ok`/`err`, `entry = "main.orl"`).

```bash
cd workspace/app
ori check main.orl
ori run main.orl
ori install demo.app --path .
```

Cache padrão: `~/.ori/packages/<name>/<version>/`.

Registry opcional (`ORI_REGISTRY` diretório ou HTTP):

```bash
ori publish . --registry /caminho/registry
ori install outro.pkg@0.1.0
```

Contrato: [registry-v1.md](../planning/registry-v1.md) (planejamento; sem push
de loja).

## Após atualizar o Ori

1. Leia o [CHANGELOG.md](../../CHANGELOG.md).
2. `ori check` / `ori test`.
3. Pré-S3: `ori migrate-syntax .`

Próximo: [Cookbook](cookbook.pt-BR.md) · [Tour](../language/tour.pt-BR.md) ·
[Instalação](../install.pt-BR.md) · [Exemplos](../../examples/)
