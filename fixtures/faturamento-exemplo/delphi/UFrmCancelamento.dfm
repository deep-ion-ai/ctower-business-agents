object FrmCancelamento: TFrmCancelamento
  Left = 300
  Top = 200
  BorderStyle = bsDialog
  Caption = 'Cancelamento de Fatura'
  ClientHeight = 260
  ClientWidth = 420
  Position = poScreenCenter
  PixelsPerInch = 96
  TextHeight = 13
  object lblFatura: TLabel
    Left = 16
    Top = 16
    Width = 80
    Height = 13
    Caption = 'Numero da fatura'
  end
  object lblMotivo: TLabel
    Left = 16
    Top = 64
    Width = 120
    Height = 13
    Caption = 'Motivo do cancelamento'
  end
  object edtFatura: TEdit
    Left = 16
    Top = 32
    Width = 160
    Height = 21
    TabOrder = 0
  end
  object memMotivo: TMemo
    Left = 16
    Top = 80
    Width = 388
    Height = 120
    MaxLength = 200
    TabOrder = 1
  end
  object btnCancelar: TButton
    Left = 236
    Top = 216
    Width = 80
    Height = 25
    Caption = 'Cancelar fatura'
    TabOrder = 2
    OnClick = btnCancelarClick
  end
  object btnFechar: TButton
    Left = 324
    Top = 216
    Width = 80
    Height = 25
    Caption = 'Fechar'
    TabOrder = 3
    OnClick = btnFecharClick
  end
  object qryFatura: TADOQuery
    Parameters = <>
    Left = 16
    Top = 216
  end
  object spCancelar: TADOStoredProc
    Parameters = <>
    Left = 56
    Top = 216
  end
end
