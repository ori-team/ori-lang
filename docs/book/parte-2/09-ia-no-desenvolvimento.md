# Cap. 9 — Desenvolver com assistência de IA

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** II

## TL;DR

A Ori é um lab explícito de **humano + agente** no mesmo código. Agentes seguem
`AGENTS.md` e skills (compiler-dev, ori-testing, living-docs). Humanos fecham
decisões de produto; a IA acelera implementação e docs — não inventa spec.

## Exemplo

Contrato mental de uma sessão com agente:

1. Ler manifesto + spec do pedaço tocado  
2. Implementar fatia pequena  
3. Teste de regressão (`ori-driver` / exemplo)  
4. CHANGELOG + docs no **mesmo** slice  
5. Não reintroduzir sintaxe pré-S3  

## Como funciona

### Papéis

| Humano | Agente |
|--------|--------|
| Decide superfície e prioridades | Executa fatias com skills |
| Aceita ADR / corte de escopo | Propõe patches e testes |
| Faz review de contrato | Atualiza catálogo/CHANGELOG quando pedido |

### Skills do projeto (obrigatórias em código)

- `clean-code` — estrutura e nomes  
- `rust` — crates e qualidade Rust  
- `living-docs` — docs com o código  
- `compiler-dev` — fase certa do front-end  
- `lang-compiled` — AOT / IR / backends  
- `ori-testing` — L1 check → L2 compile → L3 run  
- `ori-lang-qa` — matriz e FREEZE  

### O que **não** pedir à IA

- “Aceitar de novo” sintaxe pré-S3  
- Features de produto arquivadas (game/imgui como foco de linguagem)  
- Spec inventada sem bater no compilador  
- Self-hosting como próximo passo tático (M4 é último)

## O que memorizar

- `AGENTS.md` é a constituição dos agentes neste repo.
- Teste + CHANGELOG + docs andam com o código.
- IA amplifica processo; não substitui o manifesto.

## Ir mais fundo

- [`../../../AGENTS.md`](../../../AGENTS.md)
- [`../../../CONTRIBUTING.md`](../../../CONTRIBUTING.md)
