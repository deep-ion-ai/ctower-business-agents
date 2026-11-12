unit UFrmCancelamento;

{ Faturamento Exemplo - retaguarda
  Tela de cancelamento de fatura usada pelo atendimento interno. }

interface

uses
  Windows, SysUtils, Classes, Controls, Forms, Dialogs, StdCtrls, DB, ADODB, DateUtils;

type
  TFrmCancelamento = class(TForm)
    lblFatura: TLabel;
    edtFatura: TEdit;
    lblMotivo: TLabel;
    memMotivo: TMemo;
    btnCancelar: TButton;
    btnFechar: TButton;
    qryFatura: TADOQuery;
    spCancelar: TADOStoredProc;
    procedure btnCancelarClick(Sender: TObject);
    procedure btnFecharClick(Sender: TObject);
  private
    FUsuario: string;
    FPerfil: string;
    function DiasDesdeEmissao(AIdFatura: Integer): Integer;
    function ValidarCancelamento(AIdFatura: Integer): Boolean;
  public
    procedure Configurar(const AUsuario, APerfil: string);
  end;

var
  FrmCancelamento: TFrmCancelamento;

implementation

{$R *.dfm}

const
  TAM_MINIMO_MOTIVO = 10;
  DIAS_LIMITE_SEM_SUPERVISOR = 30;
  PERFIL_SUPERVISOR = 'SUPERVISOR';

procedure TFrmCancelamento.Configurar(const AUsuario, APerfil: string);
begin
  FUsuario := AUsuario;
  FPerfil := APerfil;
end;

function TFrmCancelamento.DiasDesdeEmissao(AIdFatura: Integer): Integer;
begin
  qryFatura.Close;
  qryFatura.SQL.Text := 'SELECT DT_EMISSAO FROM FATURA WHERE ID_FATURA = :ID';
  qryFatura.Parameters.ParamByName('ID').Value := AIdFatura;
  qryFatura.Open;
  Result := DaysBetween(Date, qryFatura.FieldByName('DT_EMISSAO').AsDateTime);
end;

function TFrmCancelamento.ValidarCancelamento(AIdFatura: Integer): Boolean;
begin
  Result := False;

  if Length(Trim(memMotivo.Text)) < TAM_MINIMO_MOTIVO then
  begin
    ShowMessage('Informe o motivo do cancelamento (minimo de 10 caracteres).');
    Exit;
  end;

  { Fatura emitida ha mais de 30 dias so pode ser cancelada por supervisor }
  if (DiasDesdeEmissao(AIdFatura) > DIAS_LIMITE_SEM_SUPERVISOR) and
     (FPerfil <> PERFIL_SUPERVISOR) then
  begin
    ShowMessage('Fatura emitida ha mais de 30 dias. Solicite a um supervisor.');
    Exit;
  end;

  Result := True;
end;

procedure TFrmCancelamento.btnCancelarClick(Sender: TObject);
var
  IdFatura: Integer;
begin
  IdFatura := StrToIntDef(edtFatura.Text, 0);
  if IdFatura <= 0 then
  begin
    ShowMessage('Informe o numero da fatura.');
    Exit;
  end;

  if not ValidarCancelamento(IdFatura) then
    Exit;

  spCancelar.ProcedureName := 'PKG_COBRANCA.PRC_CANCELAR_FATURA';
  spCancelar.Parameters.Refresh;
  spCancelar.Parameters.ParamByName('P_ID_FATURA').Value := IdFatura;
  spCancelar.Parameters.ParamByName('P_MOTIVO').Value := Trim(memMotivo.Text);
  spCancelar.Parameters.ParamByName('P_USUARIO').Value := FUsuario;
  spCancelar.ExecProc;

  ShowMessage('Fatura cancelada.');
  memMotivo.Clear;
  edtFatura.Clear;
end;

procedure TFrmCancelamento.btnFecharClick(Sender: TObject);
begin
  Close;
end;

end.
