import { ROTULO_ESTADO_RASTREADOR, quandoConferido, type RastreadorVista } from "./demandas";

/**
 * Estado da demanda no rastreador (#292): o estado conferido e quando, e o aviso
 * quando a última conferência falhou (a tela continua com o último estado conhecido).
 */
export function EstadoNoRastreador({
  rastreador,
  aviso,
}: {
  rastreador: RastreadorVista | null;
  aviso: string | null;
}) {
  return (
    <>
      {rastreador && (
        <span className="muted estado-rastreador">
          no rastreador: {ROTULO_ESTADO_RASTREADOR[rastreador.estado]} ·
          conferido em {quandoConferido(rastreador.conferido)}
        </span>
      )}
      {aviso && (
        <span className="aviso-rastreador" role="status">
          {rastreador ? aviso : `Status não conferido: ${aviso}`}
        </span>
      )}
    </>
  );
}
