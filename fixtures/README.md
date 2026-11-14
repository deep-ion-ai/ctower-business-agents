# Fixtures

Sistemas fictícios com regras de negócio conhecidas, usados para testar
ferramentas, prompts e telas do ctower-business-agents (spec, critério de
aceite 2). Nenhum nome, termo ou dado de sistema real entra aqui.

## Organização

```
fixtures/
  faturamento-exemplo/            # o sistema analisado
  reunioes/                       # transcripts de reuniões sobre o sistema
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

## Reuniões

Transcripts fictícios do Teams sobre o Faturamento Exemplo, para o parser de
transcript (`crates/cba-reunioes`, #134) e, depois, para a avaliação do fluxo
"Analisar reunião" (#137). Os participantes são inventados; o e-mail usa o
domínio reservado `exemplo.com`.

| Arquivo | Conteúdo |
|---|---|
| `reuniao-01-cancelamento.vtt` | Reunião 1, WebVTT como o Teams baixa (`<v Nome>`, ids de cue) |
| `reuniao-01-cancelamento.docx` | Reunião 1, `.docx` no layout do export do Teams (gerado) |
| `reuniao-01-cancelamento.txt` | Reunião 1, texto copiado da tela (`Nome  0:03` e a fala) |
| `reuniao-02-fechamento.vtt` | Reunião 2, WebVTT |

**Reunião 1** (cerca de 5 min 35 s): alinhamento sobre a trava de
cancelamento de fatura antiga (RN-FAT-014 só existe na tela Delphi). Quatro
participantes, com o nome como o Teams mostra: "Silva, Maria" (patrocinadora,
conduz), "João Pereira" (usuário-chave da retaguarda), "Ana Souza" (analista
de negócio) e "carlos.lima@exemplo.com" (desenvolvedor convidado, aparece
pelo e-mail). Tem decisões (a trava vai para o banco; o mínimo de 10
caracteres do motivo fica; o relatório mensal vira outra demanda),
necessidades (trava fora da tela Delphi; relatório mensal de cancelamentos),
uma regra citada (fatura com pagamento parcial não pode ser cancelada,
RN-FAT-012), pendências com responsável e prazo (Carlos até o dia 20, Ana
até o dia 18, Maria até o dia 25), dúvidas (os 30 dias contam da emissão ou
do vencimento; órgão público precisa de aprovação extra) e próximos passos
(análise de impacto; nova conversa no dia 27). Vai virar gabarito da
avaliação na #137.

**Reunião 2** (cerca de 2 min): fechamento do escopo. Participantes em comum
com a reunião 1, para testar o histórico de papéis: "Maria Silva" (a mesma
pessoa que "Silva, Maria" na reunião 1, com o nome em outra variação), "Ana
Souza", "João Pereira" (agora revisando como usuário da retaguarda) e uma
pessoa nova, "Beatriz Rocha" (QA).

Os três arquivos da reunião 1 têm o mesmo conteúdo: os testes de
`crates/cba-reunioes/tests/fixtures_reunioes.rs` conferem que geram as mesmas
falas (falante e texto). O `.docx` é gerado a partir do `.txt`, de forma
reproduzível (sem compressão e com data fixa). Depois de mudar o `.txt`,
regere o `.docx` e commite os dois:

```
CBA_ATUALIZAR_ESPERADOS=1 cargo test -p cba-reunioes --test fixtures_reunioes
```

Sem a variável, o teste falha se o `.docx` commitado não corresponder ao
`.txt`. O `.gitattributes` marca o `.docx` como binário.

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
  fixtures, e todo número com formato de CPF ou CNPJ é inválido (no `.docx`,
  a verificação vale para o texto dos parágrafos).

```
cd crates/cba-fixtures && cargo test
```

Ao alterar um arquivo da fixture, ajuste as linhas do gabarito e rode o
teste. O `.gitattributes` da raiz fixa o final de linha das fixtures em LF,
para as linhas serem as mesmas em qualquer sistema operacional.
