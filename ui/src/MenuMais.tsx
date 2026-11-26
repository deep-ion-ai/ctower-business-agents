import { BarraAcoes, MenuAcoes, type Acao } from "./componentes";
import { IconeReticencias } from "./icones";

export type ItemMenuMais = Pick<Acao, "texto" | "aoEscolher" | "desabilitado">;

/**
 * Botão “⋯” com um menu de ações secundárias: o {@link MenuAcoes} compartilhado (#410)
 * só com o ícone, com `rotulo` como nome acessível do botão e do menu. Fica numa
 * fileira encostada à direita para o menu abrir para a esquerda, sem sair da tela.
 */
export function MenuMais({ rotulo, itens }: { rotulo: string; itens: ItemMenuMais[] }) {
  return (
    <BarraAcoes rotulo={rotulo} alinhamento="fim">
      <MenuAcoes rotulo={rotulo} icone={<IconeReticencias />} itens={itens} />
    </BarraAcoes>
  );
}
