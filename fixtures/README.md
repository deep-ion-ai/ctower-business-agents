# Fixtures

Sistemas fictícios com regras de negócio conhecidas, usados para testar
ferramentas, prompts e telas do ctower-business-agents (spec, critério de
aceite 2). Nenhum nome, termo ou dado de sistema real entra aqui.

## Organização

```
fixtures/
  faturamento-exemplo/            # o sistema analisado
  gabaritos/
    faturamento-exemplo.yaml      # as respostas esperadas
```

A pasta do projeto e o gabarito ficam separados de propósito. Uma
ferramenta, ou um agente, que receba `faturamento-exemplo/` como projeto
cadastrado não enxerga as respostas. Não coloque dentro da pasta do projeto
nenhum arquivo que explique as regras.

## Faturamento Exemplo

Sistema legado de faturamento e cobrança da "Empresa Exemplo de Serviços
Ltda.", empresa inventada. É pequeno, mas tem o que um sistema real costuma
ter:

- várias tecnologias: scripts SQL e PL/SQL (`db/`), backend Java
  (`backend/`), tela TypeScript (`frontend/`), lote COBOL (`cobol/`) e um
  form Delphi da retaguarda (`delphi/`);
- regras de negócio espalhadas entre as camadas, com exceções e parâmetros
  lidos de tabela;
- divergências entre camadas, como acontece em projetos que evoluíram aos
  poucos;
- um `.env` falso, com valores que não funcionam em lugar algum, para testar
  a exclusão de arquivos com segredos.

O código não precisa compilar nem rodar: só precisa ser plausível e estável,
porque o gabarito cita arquivos, linhas e trechos.

## Gabarito

`gabaritos/faturamento-exemplo.yaml` descreve o que uma análise correta
deve encontrar:

- `regras`: cada regra com id, enunciado e evidências (arquivo relativo à
  raiz do projeto, linha inicial, linha final e um trecho que aparece no
  intervalo);
- `inconsistencias`: divergências entre camadas que o analista deve apontar;
- `perguntas`, `funcionalidades` e `demandas`: exemplos dos três fluxos da
  spec (perguntar ao sistema, explicar funcionalidade, analisar impacto),
  com as regras e os arquivos esperados;
- `exclusoes`: arquivos que as ferramentas devem ignorar;
- `perguntas_esperadas` e `verificacao_da_premissa`: o que a avaliação dos
  fluxos ([`docs/avaliacao.md`](../docs/avaliacao.md)) espera das perguntas
  para o solicitante e da correção da premissa da demanda.

## Testes

A crate `crates/cba-fixtures` confere a fixture:

- todo arquivo e intervalo citado no gabarito existe e contém o trecho
  esperado; ids são únicos; toda regra citada existe; os três fluxos têm
  exemplos; toda regra aparece em ao menos um fluxo;
- nenhum termo de uma lista de empresas, órgãos e sistemas reais aparece nas
  fixtures, e todo número com formato de CPF ou CNPJ é inválido.

```
cd crates/cba-fixtures && cargo test
```

Ao alterar um arquivo da fixture, ajuste as linhas do gabarito e rode o
teste. O `.gitattributes` da raiz fixa o final de linha das fixtures em LF,
para as linhas serem as mesmas em qualquer sistema operacional.
