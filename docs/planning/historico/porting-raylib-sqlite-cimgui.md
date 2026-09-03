# Plano Estratégico: Port de Bibliotecas Nativas (Raylib, SQLite, cimgui)

> **Status: ARQUIVADO / não-core.**  
> Ideias históricas para pacotes de **comunidade** (`ori-raylib`, `ori-sqlite`,
> bindings cimgui). **Não** é backlog do produto Ori. Prioridade atual: linguagem,
> docs/exemplos, performance. Não reabrir no chat como feature do monorepo.

Este documento descreve a estratégia para portar três grandes pilares do ecossistema C/C++ para a linguagem Ori. O objetivo não é apenas criar um *wrapper* 1:1, mas sim elevar a ergonomia das bibliotecas originais para abraçar a filosofia de legibilidade, tipagem forte explícita e orientação segura a dados que o Ori prega.

## 1. Raylib (ori-raylib) - Gráficos e Jogos
Raylib é puramente funcional e não possui classes ocultas, o que a torna perfeita para a sintaxe do Ori.

### O Problema Original (C):
Em C, a passagem de configurações é frequentemente feita por múltiplos parâmetros soltos ou modificação de variáveis globais ocultas.
### O Refinamento no Ori:
- **Agrupamento por Tipos Estritos:** Agruparemos configurações em Structs imutáveis nativas do Ori.
- **Gerenciamento de Memória Transparente:** Usaremos construtores explícitos e alavancaremos o suporte nativo do Ori (caso habilitado via RAII/disposes) para limpar texturas e buffers sem exigir `UnloadTexture` a todo momento.
- **Exemplo de Ergonomia Desejada:**
  ```ori
  import "raylib" as rl

  fn main() {
      let window = rl.Window.init(800, 600, "Meu Jogo Ori");
      let texture = rl.Texture.load("assets/player.png");

      while not window.should_close() {
          rl.begin_drawing();
          rl.clear_background(rl.Color.RAYWHITE);
          
          texture.draw(100, 100, rl.Color.WHITE);

          rl.end_drawing();
      }
      // Opcional: window e texture fecham a si mesmos no Drop/Dispose.
  }
  ```
- **Novas Funções Ori-Only:** Métodos auxiliares geométricos acoplados diretamente ao tipo `Vector2` e `Rectangle` do Ori para facilitar a física sem as verbosas funções soltas do C.

---

## 2. SQLite (ori-sqlite) - Banco de Dados Local
SQLite é o padrão ouro para armazenamento local, mas a API de C dele (`sqlite3_prepare_v2`, `sqlite3_step`) é extremamente verbosa e propensa a vazamentos se um *statement* não for finalizado.

### O Refinamento no Ori:
- **Iteradores Assíncronos Seguros:** Em vez de *loops* manuais com `sqlite3_step`, a abstração do Ori retornará uma Lista geradora ou um Stream Assíncrono para os dados.
- **Tratamento de Erro Elegante:** Abandono dos códigos de retorno `int` do C. Transformação automática em `Result<Value, Error>`.
- **Exemplo de Ergonomia Desejada:**
  ```ori
  import "sqlite" as db

  fn main() ! {
      let conn = try db.open("banco.db");
      try conn.execute("CREATE TABLE users (id INTEGER, name TEXT)");

      // Uso de strings formatadas e preparadas implicitamente!
      let users = try conn.query("SELECT * FROM users WHERE age > ?", [18]);
      
      for user in users {
          print("User: {user.name}");
      }
  }
  ```

---

## 3. Cimgui (ori-imgui) - Interfaces Gráficas
A biblioteca *Dear ImGui* é revolucionária, mas a FFI para ela (via *cimgui*) no Rust/C requer a passagem constante de ponteiros e lidar com estados de string complexos.

### O Refinamento no Ori:
- **Closures para Contexto:** Ao invés de forçar o usuário a sempre chamar `EndWindow()` manualmente (o que causa pânico se esquecido), podemos tirar proveito de callbacks/lambdas no Ori.
- **Exemplo de Ergonomia Desejada:**
  ```ori
  import "imgui" as ui

  fn main() {
      ui.window("Painel de Controle", || {
          if ui.button("Clique Aqui") {
              print("Botão clicado!");
          }
          ui.text("Este é um texto renderizado com facilidade.");
      }); // EndWindow() é chamado automaticamente aqui!
  }
  ```

## Conclusão de Aprimoramentos
Em todas as três bibliotecas, se o ecossistema C não fornecer o método que precisamos, criaremos **funções aglutinadoras (wrappers) diretas na lógica do Ori** para encapsular a sujeira do C. A prioridade é a DX (Developer Experience).
