# Plano de implementação — ecossistema de pacotes em produção

> **Status:** reaberto por decisão de 2026-08-09; execução posterior às
> fundações de linguagem/DX prioritárias.  
> **Baseline:** path/git/registry local ou HTTP, publish, cache e lockfile já
> existem; não há serviço oficial operado nem política completa de supply chain.

## 1. Resultado desejado

Um usuário deve conseguir descobrir, adicionar, verificar, publicar e manter
dependências com builds reproduzíveis e procedência visível.

O serviço remoto é infraestrutura separada. O compilador mantém protocolo,
cliente, validação e uma implementação de referência testável.

## 2. Lacunas atuais

- ausência de catálogo oficial/discovery;
- autenticação, ownership de nomes e recuperação de conta indefinidos;
- checksums/assinaturas/provenance de pacotes incompletos;
- política de yanking, retenção, moderação e disponibilidade ausente;
- sem canais stable/dev de toolchain ou versões lado a lado;
- mirror/offline corporativo não formalizado;
- `ori add/remove` ainda não oferece o fluxo diário esperado.

## 3. Fases

| ID | Entrega | Critério observável |
|---|---|---|
| **PKG-REG-1.0** | protocolo/schema v1 fechado | cliente e servidor de referência passam contract tests |
| **PKG-REG-1.1** | integridade/provenance | lock registra digest; conteúdo alterado é rejeitado |
| **PKG-REG-1.2** | `ori add/remove/search` | manifest/lock mudam atomicamente e possuem rollback |
| **PKG-REG-1.3** | auth e ownership | token de escopo mínimo; colisão/transferência auditáveis |
| **PKG-REG-1.4** | yank/retention/mirror | builds travados continuam reproduzíveis após yank |
| **PKG-REG-1.5** | assinatura e transparência | origem e identidade verificáveis conforme threat model |
| **PKG-REG-1.6** | operação | backup, restore, rate limit, abuse e incident runbook testados |
| **TOOLCHAIN-1.0** | canais/versões lado a lado | projeto fixa toolchain sem substituir instalação global |

## 4. Segurança

- publicação é imutável por versão;
- upload tem limites, valida archive traversal e arquivos permitidos;
- tokens nunca entram em lockfile/log;
- resolver não executa scripts de pacote durante descoberta;
- dependências Git registram commit exato;
- SBOM/provenance são aditivos e versionados;
- serviço segue threat model próprio antes de exposição pública.

## 5. Validação

- contract tests cliente/servidor e compatibilidade de uma versão anterior;
- pacote corrompido, replay, nome tomado e token revogado;
- publish concorrente e rollback de falha parcial;
- mirror/offline e restore de backup;
- lockfile idêntico em Linux/macOS/Windows;
- teste sem rede após cache materializado.

## 6. Fora de escopo inicial

- marketplace editorial;
- executar código do pacote no servidor;
- substituir repositórios Git;
- métricas de popularidade como critério técnico de confiança.
