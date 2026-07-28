#!/bin/sh
set -eu
if test "$1" = "--section"; then
  test "$2" = "css"
  test "$3" = "600519.SH,000001.SZ"
  test "$4" = "SUPERINFLOW,SUPEROUTFLOW,BIGINFLOW,BIGOUTFLOW,MIDINFLOW,MIDOUTFLOW,SMALLINFLOW,SMALLOUTFLOW"
  test "$5" = ""
  printf '%s\n' '{"records":[{"date":"2026-07-22","code":"000001.SZ","values":{"SUPERINFLOW":50,"SUPEROUTFLOW":40,"BIGINFLOW":30,"BIGOUTFLOW":20,"MIDINFLOW":20,"MIDOUTFLOW":10,"SMALLINFLOW":10,"SMALLOUTFLOW":20}},{"date":"2026-07-22","code":"600519.SH","values":{"SUPERINFLOW":100,"SUPEROUTFLOW":40,"BIGINFLOW":80,"BIGOUTFLOW":30,"MIDINFLOW":20,"MIDOUTFLOW":25,"SMALLINFLOW":10,"SMALLOUTFLOW":20}}]}'
  exit 0
fi
if test "$1" = "--history"; then
  if test "$2" = "chmc"; then
    test "$3" = "600519.SH"
    test "$4" = "DATE,TIME,OPEN,HIGH,LOW,CLOSE,VOLUME,AMOUNT"
    test "$5" = "2026-07-22"
    test "$6" = "2026-07-22"
    test "$7" = ""
    printf '%s\n' '{"records":[{"date":"2026-07-22","code":"600519.SH","values":{"DATE":"20260722","TIME":"09:30:00","OPEN":1300,"HIGH":1302,"LOW":1299,"CLOSE":1301,"VOLUME":10,"AMOUNT":13010}},{"date":"2026-07-22","code":"600519.SH","values":{"DATE":"20260722","TIME":"09:31:00","OPEN":1301,"HIGH":1303,"LOW":1300,"CLOSE":1302,"VOLUME":11,"AMOUNT":14322}},{"date":"2026-07-22","code":"600519.SH","values":{"DATE":"20260722","TIME":"09:32:00","OPEN":1302,"HIGH":1304,"LOW":1301,"CLOSE":1303,"VOLUME":12,"AMOUNT":15636}},{"date":"2026-07-22","code":"600519.SH","values":{"DATE":"20260722","TIME":"09:33:00","OPEN":1303,"HIGH":1305,"LOW":1302,"CLOSE":1304,"VOLUME":13,"AMOUNT":16952}},{"date":"2026-07-22","code":"600519.SH","values":{"DATE":"20260722","TIME":"09:34:00","OPEN":1304,"HIGH":1306,"LOW":1303,"CLOSE":1305,"VOLUME":14,"AMOUNT":18270}}]}'
    exit 0
  fi
  test "$2" = "csd"
  test "$3" = "600519.SH"
  test "$4" = "OPEN,HIGH,LOW,CLOSE,VOLUME,AMOUNT"
  test "$5" = "2026-07-20"
  test "$6" = "2026-07-22"
  test "$7" = "Period=1,AdjustFlag=1,Order=1"
  printf '%s\n' '{"records":[{"date":"2026/7/20","code":"600519.SH","values":{"OPEN":1290,"HIGH":1310,"LOW":1288,"CLOSE":1300,"VOLUME":100,"AMOUNT":130000}},{"date":"2026/7/21","code":"600519.SH","values":{"OPEN":1300,"HIGH":1320,"LOW":1298,"CLOSE":1310,"VOLUME":110,"AMOUNT":144100}},{"date":"2026/7/22","code":"600519.SH","values":{"OPEN":1310,"HIGH":1330,"LOW":1308,"CLOSE":1320,"VOLUME":120,"AMOUNT":158400}}]}'
  exit 0
fi
test "$1" = "600519.SH,000001.SZ"
if test "$2" = "TIME,NAME,NOW,PRECLOSE,OPEN,HIGH,LOW,PCTCHANGE,VOLUME,AMOUNT"; then
  printf '%s\n' '{"records":[{"date":"2026-07-22","code":"000001.SZ","values":{"TIME":"10:00:01","NAME":"平安银行","NOW":12.5,"PRECLOSE":12.2,"OPEN":12.3,"HIGH":12.6,"LOW":12.1,"PCTCHANGE":2.459,"VOLUME":200,"AMOUNT":2500}},{"date":"2026-07-22","code":"600519.SH","values":{"TIME":"10:00:00","NAME":"贵州茅台","NOW":1300,"PRECLOSE":1290,"OPEN":1295,"HIGH":1305,"LOW":1288,"PCTCHANGE":0.7752,"VOLUME":100,"AMOUNT":130000}}]}'
else
  printf '%s\n' '{"records":[{"date":"2026-07-22","code":"000001.SZ","values":{"TIME":"10:00:01","BUYPRICE1":12.49,"BUYVOLUME1":20,"SELLPRICE1":12.51,"SELLVOLUME1":30}},{"date":"2026-07-22","code":"600519.SH","values":{"TIME":"10:00:00","BUYPRICE1":1299,"BUYVOLUME1":10,"SELLPRICE1":1301,"SELLVOLUME1":11,"BUYPRICE2":1298,"BUYVOLUME2":12,"SELLPRICE2":1302,"SELLVOLUME2":13}}]}'
fi
