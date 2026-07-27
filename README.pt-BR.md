<p align="center">
  <img src="branding/ori-logo-w_text.svg" alt="Ori" width="280">
</p>

# Ori

Ori é uma linguagem de programação com foco em leitura, tipagem explícita e compilação para código nativo. O compilador é escrito em Rust e oferece compilação AOT nativa, além de uma rota JIT em processo para `ori run` quando a biblioteca dinâmica do runtime está disponível.

**Versão atual: `0.3.8`**  
**Superfície da linguagem: S3**  
**ABI nativa: `ori-native-abi-1`**  
**Maturidade: pré-1.0, em desenvolvimento ativo**

A Ori existe para estudo de compiladores, programação assistida por IA e desenvolvimento de uma linguagem e documentação com menor carga cognitiva. É um projeto técnico sério, mas ainda não reivindica a maturidade de uma linguagem industrial.

**Idiomas:** [English](README.md) · Português · [日本語](README.ja.md)

## Comece aqui

| Objetivo | Documento |
|---|---|
| Instalar a Ori | [Instalação](docs/install.pt-BR.md) |
| Aprender a linguagem | [Tour da linguagem](docs/language/tour.pt-BR.md) |
| Criar um projeto | [Primeiro projeto](docs/guides/first-project.pt-BR.md) |
| Consultar a CLI | [Referência da CLI](docs/guides/cli-reference.pt-BR.md) |
| Ler o contrato da linguagem | [Especificação](docs/spec/README.md) |
| Entender o repositório | [Início do projeto](PROJECT_START.md) |
| Navegar por toda a documentação | [ATLAS da documentação](docs/ATLAS.md) |
| Contribuir | [Contribuindo](CONTRIBUTING.md) |
| Reportar uma vulnerabilidade | [Política de segurança](SECURITY.md) |

## Exemplo da linguagem

```ori
module app.hello

import ori.io = io

divide(a: int, b: int) -> result[int, string]
    if b == 0
        return err("divisão por zero")
    end

    return ok(a / b)
end

main() -> result[void, string]
    const answer: int = try divide(84, 2)
    io.print(f"resultado: {answer}")
    return ok()
end
```

Ideias centrais:

- identidade de módulo e imports explícitos;
- contratos públicos e tipos visíveis;
- `optional[T]` para ausência;
- `result[T, E]` e `try` para falhas recuperáveis;
- structs, enums, traits, generics e pattern matching;
- limpeza determinística com `using`;
- geração de código nativo e ABI versionada;
- diagnósticos estáveis e acionáveis.

## Instalar e executar

Guia completo: [docs/install.pt-BR.md](docs/install.pt-BR.md).

Após instalar um pacote de release:

```bash
ori --version
ori doctor
ori new hello
ori run hello/main.orl
```

Para desenvolver o compilador:

```bash
cargo --manifest-path compiler/Cargo.toml check --workspace
cargo --manifest-path compiler/Cargo.toml test --workspace
cargo --manifest-path compiler/Cargo.toml run -p ori-driver -- run examples/hello/main.orl
```

O workspace Cargo fica dentro de `compiler/`.

## Estrutura do repositório

```text
compiler/       compilador, código-fonte do runtime, LSP e CLI
stdlib/         módulos Ori e documentação sidecar
runtime/        artefatos nativos preparados por plataforma
examples/       projetos executáveis e exemplos de integração
docs/           produto, arquitetura, especificação, implementação e operações
extensions/     integrações locais com editores
tools/          QA, benchmarks, pacotes, releases e ferramentas de documentação
```

A arquitetura atual está em [docs/architecture/overview.md](docs/architecture/overview.md).

## Modelo de documentação

O repositório usa uma fonte canônica por assunto:

- produto e estado atual: `docs/product/`;
- arquitetura atual: `docs/architecture/`;
- contratos normativos: `docs/spec/`;
- padrões de implementação: `docs/implementation/`;
- qualidade e conformidade: `docs/quality/`;
- segurança: `docs/security/`;
- decisões e propostas: `docs/decisions/` e `docs/rfcs/`;
- planos complexos ativos: `docs/plans/`;
- operações: `docs/operations/`;
- evidências históricas: `docs/archive/`.

Use [docs/ATLAS.md](docs/ATLAS.md) como mapa principal.

## Estado e limitações

A Ori ainda está antes da versão 1.0. O estado atual, as rotas suportadas, as prioridades e as melhorias estruturais estão em [docs/product/status.md](docs/product/status.md). As regras de compatibilidade estão na [Spec 18](docs/spec/18-stability-and-compatibility.md), e a ABI nativa está na [Spec 19](docs/spec/19-abi.md).

## Licença

A Ori é disponibilizada sob licença dupla Apache-2.0 OR MIT. Consulte [LICENSE](LICENSE), [LICENSE-APACHE](LICENSE-APACHE) e [LICENSE-MIT](LICENSE-MIT).