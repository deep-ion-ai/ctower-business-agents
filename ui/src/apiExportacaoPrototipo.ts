import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

/**
 * Exportação do protótipo de telas numa pasta HTML para compartilhar (#380,
 * `ui/src-tauri/src/prototipo.rs`): índice, uma cópia de cada tela com a
 * navegação entre elas e os tokens aprovados. Abre sem o app e sem rede.
 */
export type ExportacaoPrototipo =
  | { situacao: "exportada"; pasta: string; telas: number; semPrototipo: number }
  /** A pasta já existe no destino: nada foi gravado; confirme e chame com `substituir`. */
  | { situacao: "pasta_existe"; pasta: string };

export interface ApiExportacaoPrototipo {
  /** Diálogo de escolher pasta; `null` se o analista cancelar. */
  escolherPasta(inicial?: string): Promise<string | null>;
  exportar(
    projeto: string,
    analise: string,
    destino: string,
    substituir: boolean,
  ): Promise<ExportacaoPrototipo>;
}

export const apiExportacaoPrototipoTauri: ApiExportacaoPrototipo = {
  async escolherPasta(inicial) {
    const escolhida = await open({
      directory: true,
      multiple: false,
      title: "Exportar o protótipo para a pasta",
      defaultPath: inicial,
    });
    return typeof escolhida === "string" ? escolhida : null;
  },
  exportar: (projeto, analise, destino, substituir) =>
    invoke<ExportacaoPrototipo>("exportar_prototipo_app", {
      projeto,
      analise,
      destino,
      substituir,
    }),
};
