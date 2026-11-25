# Cuentas de juego

Un contrato en Stellar para un solo trato: vender, intercambiar o regalar una cuenta de PSN o de un juego como Summoners War.

## El problema

Dos personas quieren pasarse una cuenta. Una tiene Summoners War o una cuenta de PSN. La otra la quiere comprar, cambiar por otra cuenta, o recibirla de regalo.

Hoy el paso delicado pasa afuera, en el juego o en PlayStation: cada quien entra a su cuenta y cambia el correo por el de la otra persona. Quien cambia el correo primero, o quien paga primero, se queda esperando. Si el otro desaparece, se queda sin la cuenta o sin la plata.

## Por qué este problema

El riesgo no es el juego. Es el momento del cambio de manos. Hace falta un lugar neutro que guarde la plata hasta que las dos personas confirmen que ya entraron y que la cuenta ya no se puede recuperar con el correo anterior.

## Qué queda afuera

Este trato es una práctica, chica, de un solo acuerdo.

El contrato no entra a PSN ni a Summoners War. No guarda contraseñas, correos ni códigos. El cambio de correo lo hacen las personas, en el juego o en la consola. Aquí solo se guarda una descripción corta, por ejemplo `Summoners War` o `PSN`, y las firmas de las billeteras.

No hay catálogo, no hay varias publicaciones abiertas y no hay pantalla. Un contrato, un trato.

## Cómo funciona

Hay tres billeteras: la parte A, la parte B y un mediador. La parte A abre el trato. La plata es XLM. Los montos van en stroops: 1 XLM = 10.000.000.

El mediador es la tercera persona que desempata si una ya entregó y la otra no confirma. Mientras las dos cumplen, el mediador no hace nada.

### Venta

A es quien vende. B es quien compra.

1. B deposita el precio. El XLM queda en el contrato.
2. A cambia el correo de la cuenta y marca `marcar_entregado`.
3. B entra, cambia el correo y la clave para que A no la recupere, y marca `marcar_asegurado`.
4. Cualquiera de las dos llama `cerrar`. El precio pasa a A.

### Intercambio

Cada cuenta se entrega afuera. El contrato no puede guardar una cuenta de PSN, así que cada lado deja la misma seña.

1. A y B depositan la seña.
2. Los dos marcan `marcar_entregado` cuando ya cambiaron el correo.
3. Cada uno marca `marcar_asegurado` cuando ya entró a la cuenta que recibió y el otro ya no puede recuperarla. A asegura solo después de que B entregó, y al revés.
4. `cerrar` devuelve cada seña a su dueño.

### Regalo

El precio es 0. Nadie deposita.

1. A marca `marcar_entregado`.
2. B marca `marcar_asegurado`.
3. `cerrar` deja constancia. El XLM no se mueve.

### Si alguien no cumple

Antes de que alguien marque la entrega, A o B pueden `cancelar`. El depósito vuelve.

Si el plazo se vence y nadie entregó, A o B llaman `reclamar_reembolso` y la plata vuelve.

Si ya hubo una entrega y falta una confirmación, A o B llaman `disputar`. Solo el mediador resuelve:

- `resolver_venta` con `a_favor_del_vendedor` en verdadero paga a A. En falso, le devuelve la plata a B.
- `resolver_intercambio` usa `destino`: `0` devuelve cada seña, `1` se las queda A, `2` se las queda B.

En un regalo, `resolver_venta` no mueve XLM. Verdadero cierra el trato. Falso lo cancela.

## Cómo correrlo

Hace falta Rust 1.84 o más nuevo, el target `wasm32v1-none` y, para desplegar, el [Stellar CLI](https://developers.stellar.org/docs/tools/cli/install-cli).

Desde esta carpeta:

```sh
cargo test
```

Eso corre el contrato en local, sin red. Es la forma de probarlo.

Para compilar el Wasm:

```sh
rustup target add wasm32v1-none
stellar contract build
```

El archivo queda en `target/wasm32v1-none/release/cuenta_juego.wasm`.

### Desplegar en testnet

Hace falta una identidad con XLM de testnet. No guardes la llave secreta en este repositorio.

```sh
stellar keys generate --network testnet vendedor
stellar keys generate --network testnet comprador
stellar keys generate --network testnet mediador
stellar keys fund vendedor --network testnet
stellar keys fund comprador --network testnet
stellar keys fund mediador --network testnet

stellar contract deploy \
  --wasm target/wasm32v1-none/release/cuenta_juego.wasm \
  --source-account vendedor \
  --network testnet \
  --alias cuenta-juego
```

La dirección del XLM nativo en testnet:

```sh
stellar contract id asset --asset native --network testnet
```

Abrir una venta de 10 XLM. `plazo` es un timestamp de Unix posterior a hoy. `CDLZ...` es la dirección que devolvió el comando anterior.

```sh
stellar contract invoke \
  --id cuenta-juego \
  --source-account vendedor \
  --network testnet \
  -- \
  iniciar \
  --parte_a vendedor \
  --parte_b comprador \
  --mediador mediador \
  --token CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC \
  --tipo Venta \
  --precio 100000000 \
  --descripcion "Summoners War" \
  --plazo 1893456000
```

Seguir el trato:

```sh
stellar contract invoke --id cuenta-juego --source-account comprador --network testnet -- depositar --quien comprador
stellar contract invoke --id cuenta-juego --source-account vendedor --network testnet -- marcar_entregado --quien vendedor
stellar contract invoke --id cuenta-juego --source-account comprador --network testnet -- marcar_asegurado --quien comprador
stellar contract invoke --id cuenta-juego --source-account vendedor --network testnet -- cerrar --quien vendedor
```

`trato` muestra el estado, sin firmar.

## Pruebas

`cargo test` cubre:

- `venta_feliz`: el vendedor recibe el precio solo después de `marcar_entregado` y `marcar_asegurado`.
- `venta_no_cierra_sin_asegurar`: sin el asegurado, `cerrar` falla y el XLM sigue en el contrato.
- `cancelar_antes_de_entregar_devuelve_xlm`: cancelar antes de la entrega devuelve el depósito.
- `despues_de_entregar_el_comprador_no_cancela`: después de la entrega, el comprador no cancela solo.
- `intercambio_feliz`: las dos señas vuelven solo cuando están los cuatro checks.
- `regalo_cierra_sin_mover_xlm`: el regalo cierra y los balances no cambian.
- `ajeno_no_marca_ni_resuelve`: una billetera de afuera no marca ni resuelve.
- `mediador_resuelve_a_favor_del_comprador` y `mediador_resuelve_a_favor_del_vendedor`: el mediador devuelve la plata o se la paga al vendedor.
- `intercambio_en_disputa_paga_a_quien_cumplio`: en un intercambio trabado, el mediador puede dejar las señas del lado que sí entregó.
