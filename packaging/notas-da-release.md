<!-- cba:instalacao -->
## Instalação

**Recomendado: instalação por script.** Baixa o instalador desta versão, confere a soma SHA-256 e instala o app e as dependências, sem o aviso do navegador para arquivos baixados.

macOS (Terminal):

```sh
curl -fsSL https://github.com/deep-ion-ai/ctower-business-agents/releases/latest/download/install.sh | sh
```

Windows (PowerShell, sem administrador):

```powershell
irm https://github.com/deep-ion-ai/ctower-business-agents/releases/latest/download/install.ps1 | iex
```

### Instalador baixado pelo navegador

Os instaladores ainda não são assinados (a assinatura chega antes do piloto), então o sistema avisa na primeira abertura:

- **Windows** (`cba-windows-x64-setup.exe`, instala só para o seu usuário, sem administrador): na tela azul do SmartScreen ("O Windows protegeu o computador"), clique em **Mais informações** e depois em **Executar assim mesmo**.
- **macOS** (`cba-macos-universal.dmg`, Apple Silicon e Intel): abra o DMG e arraste o **Ctower Business Agents** para **Aplicativos**. Na primeira vez, clique com o botão direito (ou Control + clique) no app e escolha **Abrir**, depois **Abrir** de novo. Se o macOS só oferecer "Mover para o Lixo", abra **Ajustes do Sistema > Privacidade e Segurança**, role até o aviso sobre o app e clique em **Abrir Mesmo Assim**.

Para conferir um arquivo baixado, compare a soma com a do `SHA256SUMS`: `shasum -a 256 <arquivo>` no macOS ou `Get-FileHash <arquivo>` no PowerShell.
