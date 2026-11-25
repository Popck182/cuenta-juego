#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{StellarAssetClient, TokenClient},
    Address, Env, String,
};

const AHORA: u64 = 1_000;
const PLAZO: u64 = 10_000;
const PRECIO: i128 = 100_000_000;
const FONDO: i128 = 500_000_000;

struct Mesa<'a> {
    env: &'a Env,
    client: CuentaJuegoClient<'a>,
    id: Address,
    token_id: Address,
    token: TokenClient<'a>,
    a: Address,
    b: Address,
    mediador: Address,
    ajeno: Address,
}

fn mesa(env: &Env) -> Mesa<'_> {
    env.mock_all_auths();
    env.ledger().with_mut(|info| {
        info.timestamp = AHORA;
    });

    let emisor = Address::generate(env);
    let sac = env.register_stellar_asset_contract_v2(emisor);
    let token_id = sac.address();
    let token = TokenClient::new(env, &token_id);
    let admin = StellarAssetClient::new(env, &token_id);

    let a = Address::generate(env);
    let b = Address::generate(env);
    let mediador = Address::generate(env);
    let ajeno = Address::generate(env);
    admin.mint(&a, &FONDO);
    admin.mint(&b, &FONDO);

    let id = env.register(CuentaJuego, ());
    let client = CuentaJuegoClient::new(env, &id);
    Mesa {
        env,
        client,
        id,
        token_id,
        token,
        a,
        b,
        mediador,
        ajeno,
    }
}

fn abrir(m: &Mesa, tipo: Tipo, precio: i128, descripcion: &str) {
    m.client.iniciar(
        &m.a,
        &m.b,
        &m.mediador,
        &m.token_id,
        &tipo,
        &precio,
        &String::from_str(m.env, descripcion),
        &PLAZO,
    );
}

#[test]
fn venta_feliz() {
    let env = Env::default();
    let m = mesa(&env);
    abrir(&m, Tipo::Venta, PRECIO, "Summoners War");

    m.client.depositar(&m.b);
    m.client.marcar_entregado(&m.a);
    m.client.marcar_asegurado(&m.b);
    m.client.cerrar(&m.a);

    assert_eq!(m.token.balance(&m.a), FONDO + PRECIO);
    assert_eq!(m.token.balance(&m.b), FONDO - PRECIO);
    assert_eq!(m.token.balance(&m.id), 0);
    assert_eq!(m.client.trato().estado, Estado::Cerrado);
}

#[test]
fn venta_no_cierra_sin_asegurar() {
    let env = Env::default();
    let m = mesa(&env);
    abrir(&m, Tipo::Venta, PRECIO, "PSN");

    m.client.depositar(&m.b);
    m.client.marcar_entregado(&m.a);

    assert_eq!(
        m.client.try_cerrar(&m.a),
        Err(Ok(Error::EstadoIncorrecto))
    );
    assert_eq!(m.token.balance(&m.a), FONDO);
    assert_eq!(m.token.balance(&m.id), PRECIO);
}

#[test]
fn cancelar_antes_de_entregar_devuelve_xlm() {
    let env = Env::default();
    let m = mesa(&env);
    abrir(&m, Tipo::Venta, PRECIO, "PSN");

    m.client.depositar(&m.b);
    m.client.cancelar(&m.b);

    assert_eq!(m.token.balance(&m.b), FONDO);
    assert_eq!(m.token.balance(&m.id), 0);
    assert_eq!(m.client.trato().estado, Estado::Cancelado);
}

#[test]
fn despues_de_entregar_el_comprador_no_cancela() {
    let env = Env::default();
    let m = mesa(&env);
    abrir(&m, Tipo::Venta, PRECIO, "Summoners War");

    m.client.depositar(&m.b);
    m.client.marcar_entregado(&m.a);

    assert_eq!(
        m.client.try_cancelar(&m.b),
        Err(Ok(Error::NoSePuedeCancelar))
    );
    assert_eq!(m.token.balance(&m.b), FONDO - PRECIO);
    assert_eq!(m.token.balance(&m.id), PRECIO);
}

#[test]
fn intercambio_feliz() {
    let env = Env::default();
    let m = mesa(&env);
    abrir(&m, Tipo::Intercambio, PRECIO, "PSN");

    m.client.depositar(&m.a);
    m.client.depositar(&m.b);
    assert_eq!(
        m.client.try_cerrar(&m.a),
        Err(Ok(Error::EstadoIncorrecto))
    );

    m.client.marcar_entregado(&m.a);
    m.client.marcar_entregado(&m.b);
    m.client.marcar_asegurado(&m.b);
    assert_eq!(
        m.client.try_cerrar(&m.a),
        Err(Ok(Error::EstadoIncorrecto))
    );

    m.client.marcar_asegurado(&m.a);
    m.client.cerrar(&m.b);

    assert_eq!(m.token.balance(&m.a), FONDO);
    assert_eq!(m.token.balance(&m.b), FONDO);
    assert_eq!(m.token.balance(&m.id), 0);
    assert_eq!(m.client.trato().estado, Estado::Cerrado);
}

#[test]
fn regalo_cierra_sin_mover_xlm() {
    let env = Env::default();
    let m = mesa(&env);
    abrir(&m, Tipo::Regalo, 0, "Summoners War");

    m.client.marcar_entregado(&m.a);
    m.client.marcar_asegurado(&m.b);
    m.client.cerrar(&m.b);

    assert_eq!(m.token.balance(&m.a), FONDO);
    assert_eq!(m.token.balance(&m.b), FONDO);
    assert_eq!(m.token.balance(&m.id), 0);
    assert_eq!(m.client.trato().estado, Estado::Cerrado);
}

#[test]
fn ajeno_no_marca_ni_resuelve() {
    let env = Env::default();
    let m = mesa(&env);
    abrir(&m, Tipo::Venta, PRECIO, "PSN");

    m.client.depositar(&m.b);
    assert_eq!(
        m.client.try_marcar_entregado(&m.ajeno),
        Err(Ok(Error::NoEsParte))
    );

    m.client.marcar_entregado(&m.a);
    m.client.disputar(&m.b);
    assert_eq!(
        m.client.try_resolver_venta(&m.ajeno, &true),
        Err(Ok(Error::NoEsMediador))
    );
    assert_eq!(m.token.balance(&m.id), PRECIO);
}

#[test]
fn mediador_resuelve_a_favor_del_comprador() {
    let env = Env::default();
    let m = mesa(&env);
    abrir(&m, Tipo::Venta, PRECIO, "Summoners War");

    m.client.depositar(&m.b);
    m.client.marcar_entregado(&m.a);
    m.client.disputar(&m.b);
    m.client.resolver_venta(&m.mediador, &false);

    assert_eq!(m.token.balance(&m.a), FONDO);
    assert_eq!(m.token.balance(&m.b), FONDO);
    assert_eq!(m.token.balance(&m.id), 0);
    assert_eq!(m.client.trato().estado, Estado::Cancelado);
}

#[test]
fn mediador_resuelve_a_favor_del_vendedor() {
    let env = Env::default();
    let m = mesa(&env);
    abrir(&m, Tipo::Venta, PRECIO, "PSN");

    m.client.depositar(&m.b);
    m.client.marcar_entregado(&m.a);
    m.client.disputar(&m.a);
    m.client.resolver_venta(&m.mediador, &true);

    assert_eq!(m.token.balance(&m.a), FONDO + PRECIO);
    assert_eq!(m.token.balance(&m.b), FONDO - PRECIO);
    assert_eq!(m.token.balance(&m.id), 0);
    assert_eq!(m.client.trato().estado, Estado::Cerrado);
}

#[test]
fn intercambio_en_disputa_paga_a_quien_cumplio() {
    let env = Env::default();
    let m = mesa(&env);
    abrir(&m, Tipo::Intercambio, PRECIO, "PSN");

    m.client.depositar(&m.a);
    m.client.depositar(&m.b);
    m.client.marcar_entregado(&m.a);
    m.client.marcar_asegurado(&m.b);
    m.client.disputar(&m.a);
    m.client.resolver_intercambio(&m.mediador, &1);

    assert_eq!(m.token.balance(&m.a), FONDO + PRECIO);
    assert_eq!(m.token.balance(&m.b), FONDO - PRECIO);
    assert_eq!(m.token.balance(&m.id), 0);
}
