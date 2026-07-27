# Redefinicao do C Backend

> Status: **archived**  
> Original path: `docs/planning/historico/c-backend-redefinition.md`  
> Original date: before 2026-07-27  
> Archived on: 2026-07-27  
> Category: `investigations`  
> Current replacement: `docs/spec/14-backend-support.md`  
> Warning: versions, syntax, commands, paths, and priorities below may be obsolete.


> Status: implementado; escopo síncrono permanente decidido em 2026-07-23
> Data: 2026-06-30
> Escopo: registro histórico da redefinição. O contrato vigente está em
> `docs/spec/14-backend-support.md`.

## Decisao proposta

O backend nativo Cranelift deve continuar sendo a referencia semantica da Ori.
O backend C deixa de ser apresentado como rota normal de build.

O comando `ori build` deve ser redefinido para significar "construir um projeto
Ori" pela rota principal da linguagem. A emissao de C, se continuar existindo,
deve ficar atras de um comando explicito de debug, por exemplo:

```bash
ori emit c <file.orl>
```

ou:

```bash
ori debug c <file.orl>
```

A segunda etapa foi encerrada em 2026-07-23: a emissão C continua disponível
como ferramenta de debug/transpilação síncrona, sem promessa de paridade.
Corrigem-se crashes, C inválido e semântica incorreta dentro do subconjunto
documentado; recursos novos podem emitir `backend.c_unsupported`.

## Por que mudar

Hoje a matriz C cria uma promessa maior do que o backend entrega. Isso aumenta
custo de manutencao e confunde a leitura do projeto: o nativo e a verdade, mas
o C ainda aparece em docs e testes como se fosse uma rota quase equivalente.

Para a filosofia da Ori, isso e ruim por dois motivos:

- aumenta carga cognitiva para quem esta lendo a linguagem;
- obriga cada feature nova a responder duas vezes a mesma pergunta: "funciona no
  nativo?" e "funciona no C parcial?".

## Pros

- Menos superficie falsa: `ori build` passa a apontar para o caminho real.
- Menos manutencao duplicada em async, ARC, colecoes, stdlib e FFI.
- Testes ficam mais proximos do contrato da linguagem.
- Documentacao fica mais honesta: backend C vira ferramenta de debug, nao
  promessa de portabilidade.
- O projeto pode focar em FFI, pacotes, concorrencia e runtime nativo.

## Contras

- Perde-se uma forma simples de inspecionar saida C gerada.
- Alguns testes `build_c_backend_*` precisam virar testes nativos, HIR, checker
  ou testes especificos de "emit C".
- O historico de paridade C x stdlib precisa ser limpo com cuidado para nao
  esconder regressao real.
- Usuarios que tenham usado `ori build` para C terao uma breaking change.
- Se a emissao C for removida totalmente, perdemos um experimento de
  portabilidade que poderia ser util no futuro.

## Mudancas planejadas

1. Redefinir `ori build`.
   - `ori build <path>` deve construir projeto/arquivo pela rota principal.
   - `ori compile` pode continuar como rota AOT explicita.
   - A diferenca entre `build`, `compile`, `run` e `test` precisa estar clara no
     `README.md`, no help da CLI e na spec.

2. Isolar ou remover a emissao C.
   - Opcao A: manter como `ori emit c` ou `ori debug c`.
   - Opcao B: remover do fluxo publico e manter apenas helpers internos por um
     ciclo.
   - Opcao C: remover codigo e testes C depois de migrar cobertura critica.

3. Reescrever testes.
   - Migrar `build_c_backend_*` que validam semantica da linguagem para testes
     nativos.
   - Manter poucos testes de C apenas se o comando de debug continuar existindo.
   - Trocar testes de matriz C por testes de contrato: manifesto stdlib, native
     ABI, checker e diagnosticos.

4. Limpar matriz C.
   - `docs/spec/14-backend-support.md` deve deixar de vender paridade C.
   - A matriz pode virar historico, nota de debug ou ser removida.
   - `docs/spec/15-stdlib-maintenance.md` deve parar de exigir `c_backend` para
     novas funcoes, exceto se o comando C for mantido explicitamente.

5. Atualizar changelog e guias.
   - Registrar a redefinicao como breaking change quando ela for implementada.
   - Explicar como migrar de `ori build` antigo para o novo comando.

## Criterio de aceite

- `ori build` nao significa mais "emitir C" em nenhuma documentacao publica.
- Testes semanticos importantes nao dependem do backend C.
- Se ainda existir emissao C, ela aparece como debug parcial e tem comando
  proprio.
- A matriz C nao bloqueia features novas da linguagem.

## Norte recomendado

Recomendo manter a emissao C por um ciclo como comando de debug explicito, sem
promessa de paridade. Depois disso, se ela continuar consumindo manutencao sem
beneficio claro, remover.

Isso reduz risco: a linguagem ganha clareza agora, mas o projeto ainda tem uma
janela para aproveitar qualquer utilidade real do backend C antes de apagar.
