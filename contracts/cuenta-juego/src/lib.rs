#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, Env, String,
};

/// Venta: B deposita el precio y A entrega la cuenta.
/// Intercambio: A y B depositan la misma seña.
/// Regalo: no hay plata, solo las confirmaciones.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tipo {
    Venta,
    Intercambio,
    Regalo,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Estado {
    Abierto,
    Fondeado,
    EnCurso,
    Listo,
    Disputa,
    Cerrado,
    Cancelado,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trato {
    pub tipo: Tipo,
    pub parte_a: Address,
    pub parte_b: Address,
    pub mediador: Address,
    pub token: Address,
    pub precio: i128,
    pub descripcion: String,
    pub plazo: u64,
    pub a_entrego: bool,
    pub b_entrego: bool,
    pub a_aseguro: bool,
    pub b_aseguro: bool,
    pub a_deposito: i128,
    pub b_deposito: i128,
    pub estado: Estado,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    YaIniciado = 1,
    NoIniciado = 2,
    MontoInvalido = 3,
    PartesInvalidas = 4,
    NoEsParte = 5,
    NoEsMediador = 6,
    EstadoIncorrecto = 7,
    YaDeposito = 8,
    DepositoNoAplica = 9,
    FaltaEntrega = 10,
    PlazoNoVencido = 11,
    YaMarco = 12,
    NoSePuedeCancelar = 13,
    ResolucionInvalida = 14,
    DescripcionVacia = 15,
    PlazoInvalido = 16,
    NoCorresponde = 17,
}

#[contracttype]
enum Clave {
    Trato,
}

const UMBRAL_TTL: u32 = 100_000;
const EXTENDER_TTL: u32 = 518_400;

#[contract]
pub struct CuentaJuego;

#[contractimpl]
impl CuentaJuego {
    /// A abre un solo trato. `precio` es el valor de la venta, la seña de cada
    /// lado en un intercambio, o 0 si es un regalo. El plazo es un timestamp.
    pub fn iniciar(
        env: Env,
        parte_a: Address,
        parte_b: Address,
        mediador: Address,
        token: Address,
        tipo: Tipo,
        precio: i128,
        descripcion: String,
        plazo: u64,
    ) -> Result<(), Error> {
        parte_a.require_auth();
        if env.storage().instance().has(&Clave::Trato) {
            return Err(Error::YaIniciado);
        }
        if parte_a == parte_b || mediador == parte_a || mediador == parte_b {
            return Err(Error::PartesInvalidas);
        }
        if descripcion.len() == 0 {
            return Err(Error::DescripcionVacia);
        }
        if plazo <= env.ledger().timestamp() {
            return Err(Error::PlazoInvalido);
        }
        match tipo {
            Tipo::Regalo => {
                if precio != 0 {
                    return Err(Error::MontoInvalido);
                }
            }
            Tipo::Venta | Tipo::Intercambio => {
                if precio <= 0 {
                    return Err(Error::MontoInvalido);
                }
            }
        }

        guardar(
            &env,
            &Trato {
                tipo,
                parte_a,
                parte_b,
                mediador,
                token,
                precio,
                descripcion,
                plazo,
                a_entrego: false,
                b_entrego: false,
                a_aseguro: false,
                b_aseguro: false,
                a_deposito: 0,
                b_deposito: 0,
                estado: Estado::Abierto,
            },
        );
        Ok(())
    }

    pub fn depositar(env: Env, quien: Address) -> Result<(), Error> {
        quien.require_auth();
        let mut trato = cargar(&env)?;
        if trato.estado != Estado::Abierto {
            return Err(Error::EstadoIncorrecto);
        }
        let es_a = lado(&trato, &quien)?;
        match trato.tipo {
            Tipo::Regalo => return Err(Error::DepositoNoAplica),
            Tipo::Venta => {
                if es_a {
                    return Err(Error::NoCorresponde);
                }
                if trato.b_deposito != 0 {
                    return Err(Error::YaDeposito);
                }
                trato.b_deposito = trato.precio;
                trato.estado = Estado::Fondeado;
            }
            Tipo::Intercambio => {
                if es_a {
                    if trato.a_deposito != 0 {
                        return Err(Error::YaDeposito);
                    }
                    trato.a_deposito = trato.precio;
                } else {
                    if trato.b_deposito != 0 {
                        return Err(Error::YaDeposito);
                    }
                    trato.b_deposito = trato.precio;
                }
                if trato.a_deposito == trato.precio && trato.b_deposito == trato.precio {
                    trato.estado = Estado::Fondeado;
                }
            }
        }
        let token = trato.token.clone();
        let precio = trato.precio;
        guardar(&env, &trato);
        cobrar(&env, &token, &quien, precio);
        Ok(())
    }

    pub fn marcar_entregado(env: Env, quien: Address) -> Result<(), Error> {
        quien.require_auth();
        let mut trato = cargar(&env)?;
        exigir_marca(&trato)?;
        let es_a = lado(&trato, &quien)?;
        match trato.tipo {
            Tipo::Venta | Tipo::Regalo => {
                if !es_a {
                    return Err(Error::NoCorresponde);
                }
                if trato.a_entrego {
                    return Err(Error::YaMarco);
                }
                trato.a_entrego = true;
            }
            Tipo::Intercambio => {
                if es_a {
                    if trato.a_entrego {
                        return Err(Error::YaMarco);
                    }
                    trato.a_entrego = true;
                } else {
                    if trato.b_entrego {
                        return Err(Error::YaMarco);
                    }
                    trato.b_entrego = true;
                }
            }
        }
        avanzar(&mut trato);
        guardar(&env, &trato);
        Ok(())
    }

    pub fn marcar_asegurado(env: Env, quien: Address) -> Result<(), Error> {
        quien.require_auth();
        let mut trato = cargar(&env)?;
        exigir_marca(&trato)?;
        let es_a = lado(&trato, &quien)?;
        match trato.tipo {
            Tipo::Venta | Tipo::Regalo => {
                if es_a {
                    return Err(Error::NoCorresponde);
                }
                if !trato.a_entrego {
                    return Err(Error::FaltaEntrega);
                }
                if trato.b_aseguro {
                    return Err(Error::YaMarco);
                }
                trato.b_aseguro = true;
            }
            Tipo::Intercambio => {
                if es_a {
                    if !trato.b_entrego {
                        return Err(Error::FaltaEntrega);
                    }
                    if trato.a_aseguro {
                        return Err(Error::YaMarco);
                    }
                    trato.a_aseguro = true;
                } else {
                    if !trato.a_entrego {
                        return Err(Error::FaltaEntrega);
                    }
                    if trato.b_aseguro {
                        return Err(Error::YaMarco);
                    }
                    trato.b_aseguro = true;
                }
            }
        }
        avanzar(&mut trato);
        guardar(&env, &trato);
        Ok(())
    }

    pub fn cerrar(env: Env, quien: Address) -> Result<(), Error> {
        quien.require_auth();
        let mut trato = cargar(&env)?;
        lado(&trato, &quien)?;
        if trato.estado != Estado::Listo {
            return Err(Error::EstadoIncorrecto);
        }
        let tipo = trato.tipo;
        let token = trato.token.clone();
        let parte_a = trato.parte_a.clone();
        let parte_b = trato.parte_b.clone();
        let a_monto = trato.a_deposito;
        let b_monto = trato.b_deposito;
        trato.a_deposito = 0;
        trato.b_deposito = 0;
        trato.estado = Estado::Cerrado;
        guardar(&env, &trato);
        match tipo {
            Tipo::Venta => pagar(&env, &token, &parte_a, b_monto),
            Tipo::Intercambio => {
                pagar(&env, &token, &parte_a, a_monto);
                pagar(&env, &token, &parte_b, b_monto);
            }
            Tipo::Regalo => {}
        }
        Ok(())
    }

    pub fn cancelar(env: Env, quien: Address) -> Result<(), Error> {
        quien.require_auth();
        let trato = cargar(&env)?;
        lado(&trato, &quien)?;
        if trato.a_entrego || trato.b_entrego {
            return Err(Error::NoSePuedeCancelar);
        }
        if trato.estado != Estado::Abierto && trato.estado != Estado::Fondeado {
            return Err(Error::EstadoIncorrecto);
        }
        devolver(&env, trato);
        Ok(())
    }

    /// Si el plazo se vence y nadie marcó la entrega, la plata vuelve.
    pub fn reclamar_reembolso(env: Env, quien: Address) -> Result<(), Error> {
        quien.require_auth();
        let trato = cargar(&env)?;
        lado(&trato, &quien)?;
        if env.ledger().timestamp() < trato.plazo {
            return Err(Error::PlazoNoVencido);
        }
        if trato.a_entrego || trato.b_entrego {
            return Err(Error::NoSePuedeCancelar);
        }
        if trato.estado != Estado::Abierto && trato.estado != Estado::Fondeado {
            return Err(Error::EstadoIncorrecto);
        }
        devolver(&env, trato);
        Ok(())
    }

    pub fn disputar(env: Env, quien: Address) -> Result<(), Error> {
        quien.require_auth();
        let mut trato = cargar(&env)?;
        lado(&trato, &quien)?;
        if trato.estado != Estado::EnCurso {
            return Err(Error::EstadoIncorrecto);
        }
        trato.estado = Estado::Disputa;
        guardar(&env, &trato);
        Ok(())
    }

    /// El mediador suelta la venta al vendedor o se la devuelve al comprador.
    /// En un regalo no mueve XLM: solo cierra o cancela.
    pub fn resolver_venta(
        env: Env,
        quien: Address,
        a_favor_del_vendedor: bool,
    ) -> Result<(), Error> {
        quien.require_auth();
        let mut trato = cargar(&env)?;
        if quien != trato.mediador {
            return Err(Error::NoEsMediador);
        }
        if trato.estado != Estado::Disputa {
            return Err(Error::EstadoIncorrecto);
        }
        if matches!(trato.tipo, Tipo::Intercambio) {
            return Err(Error::NoCorresponde);
        }
        let tipo = trato.tipo;
        let token = trato.token.clone();
        let destino = if a_favor_del_vendedor {
            trato.parte_a.clone()
        } else {
            trato.parte_b.clone()
        };
        let monto = trato.b_deposito;
        trato.a_deposito = 0;
        trato.b_deposito = 0;
        trato.estado = if a_favor_del_vendedor {
            Estado::Cerrado
        } else {
            Estado::Cancelado
        };
        guardar(&env, &trato);
        if matches!(tipo, Tipo::Venta) {
            pagar(&env, &token, &destino, monto);
        }
        Ok(())
    }

    /// destino 0 devuelve cada seña a su dueño. 1 se las queda A. 2 se las queda B.
    pub fn resolver_intercambio(env: Env, quien: Address, destino: u32) -> Result<(), Error> {
        quien.require_auth();
        let mut trato = cargar(&env)?;
        if quien != trato.mediador {
            return Err(Error::NoEsMediador);
        }
        if trato.estado != Estado::Disputa {
            return Err(Error::EstadoIncorrecto);
        }
        if !matches!(trato.tipo, Tipo::Intercambio) {
            return Err(Error::NoCorresponde);
        }
        if destino > 2 {
            return Err(Error::ResolucionInvalida);
        }
        let token = trato.token.clone();
        let parte_a = trato.parte_a.clone();
        let parte_b = trato.parte_b.clone();
        let a_monto = trato.a_deposito;
        let b_monto = trato.b_deposito;
        let total = a_monto.checked_add(b_monto).ok_or(Error::MontoInvalido)?;
        trato.a_deposito = 0;
        trato.b_deposito = 0;
        trato.estado = Estado::Cerrado;
        guardar(&env, &trato);
        match destino {
            0 => {
                pagar(&env, &token, &parte_a, a_monto);
                pagar(&env, &token, &parte_b, b_monto);
            }
            1 => pagar(&env, &token, &parte_a, total),
            _ => pagar(&env, &token, &parte_b, total),
        }
        Ok(())
    }

    pub fn trato(env: Env) -> Result<Trato, Error> {
        cargar(&env)
    }
}

fn exigir_marca(trato: &Trato) -> Result<(), Error> {
    let permitido = match trato.tipo {
        Tipo::Regalo => matches!(trato.estado, Estado::Abierto | Estado::EnCurso),
        Tipo::Venta | Tipo::Intercambio => {
            matches!(trato.estado, Estado::Fondeado | Estado::EnCurso)
        }
    };
    if permitido {
        Ok(())
    } else {
        Err(Error::EstadoIncorrecto)
    }
}

fn avanzar(trato: &mut Trato) {
    let listo = match trato.tipo {
        Tipo::Venta | Tipo::Regalo => trato.a_entrego && trato.b_aseguro,
        Tipo::Intercambio => {
            trato.a_entrego && trato.b_entrego && trato.a_aseguro && trato.b_aseguro
        }
    };
    if listo {
        trato.estado = Estado::Listo;
    } else if trato.a_entrego || trato.b_entrego || trato.a_aseguro || trato.b_aseguro {
        trato.estado = Estado::EnCurso;
    }
}

fn devolver(env: &Env, mut trato: Trato) {
    let token = trato.token.clone();
    let parte_a = trato.parte_a.clone();
    let parte_b = trato.parte_b.clone();
    let a_monto = trato.a_deposito;
    let b_monto = trato.b_deposito;
    trato.a_deposito = 0;
    trato.b_deposito = 0;
    trato.estado = Estado::Cancelado;
    guardar(env, &trato);
    pagar(env, &token, &parte_a, a_monto);
    pagar(env, &token, &parte_b, b_monto);
}

fn cobrar(env: &Env, token_id: &Address, de: &Address, monto: i128) {
    token::TokenClient::new(env, token_id).transfer(de, &env.current_contract_address(), &monto);
}

fn pagar(env: &Env, token_id: &Address, hacia: &Address, monto: i128) {
    if monto <= 0 {
        return;
    }
    token::TokenClient::new(env, token_id).transfer(&env.current_contract_address(), hacia, &monto);
}

fn guardar(env: &Env, trato: &Trato) {
    env.storage().instance().set(&Clave::Trato, trato);
    env.storage().instance().extend_ttl(UMBRAL_TTL, EXTENDER_TTL);
}

fn cargar(env: &Env) -> Result<Trato, Error> {
    env.storage()
        .instance()
        .get(&Clave::Trato)
        .ok_or(Error::NoIniciado)
}

/// `true` si `quien` es A. `false` si es B.
fn lado(trato: &Trato, quien: &Address) -> Result<bool, Error> {
    if quien == &trato.parte_a {
        Ok(true)
    } else if quien == &trato.parte_b {
        Ok(false)
    } else {
        Err(Error::NoEsParte)
    }
}

mod test;
