// Importação de arquivo como texto (`?raw` do Vite), usada nos testes.
declare module "*?raw" {
  const conteudo: string;
  export default conteudo;
}
