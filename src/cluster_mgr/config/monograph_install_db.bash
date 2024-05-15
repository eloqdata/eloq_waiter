#!/bin/bash
function monograph_install_db() {
  printf "MonographDB Install Database Start. \n"
  printf "INSTALL_DIR=%s\n" ${INSTALL_DIR}
  printf "DATA_DIR=%s\n" ${DATA_DIR}
  mkdir -p ${DATA_DIR}
  ${MALLOC}
  export LD_LIBRARY_PATH=${INSTALL_DIR}/lib:$LD_LIBRARY_PATH
  init_db_script="${INSTALL_DIR}/scripts/mysql_install_db \
    --defaults-file=${BS_INI} --basedir=${INSTALL_DIR} \
    --datadir=${DATA_DIR} \
    --plugin-dir=${INSTALL_DIR}/lib/plugin --skip-test-db"
  echo "$init_db_script"
  eval "$init_db_script"
}

monograph_install_db
