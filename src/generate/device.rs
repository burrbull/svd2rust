use quote::ToTokens;
use proc_macro2::{TokenStream, Ident, Span};
use std::fs::File;
use std::io::Write;
use crate::svd::Device;

use crate::errors::*;
use crate::util::{self, ToSanitizedSnakeCase, ToSanitizedUpperCase};
use crate::Target;

use crate::generate::{interrupt, peripheral};

/// A collection of Tokens and available feature flags
pub struct RenderOutput {
    pub tokens: Vec<TokenStream>,
    pub features: Vec<String>,
}

/// Whole device generation
pub fn render(
    d: &Device,
    target: Target,
    nightly: bool,
    generic_mod: bool,
    conditional: bool,
    device_x: &mut String,
) -> Result<RenderOutput> {
    let mut output = RenderOutput {
        tokens: vec![],
        features: vec![],
    };

    let doc = format!(
        "Peripheral access API for {0} microcontrollers \
         (generated using svd2rust v{1})\n\n\
         You can find an overview of the API [here].\n\n\
         [here]: https://docs.rs/svd2rust/{1}/svd2rust/#peripheral-api",
        d.name.to_uppercase(),
        env!("CARGO_PKG_VERSION")
    );

    if target == Target::Msp430 {
        output.tokens.push(quote! {
            #![feature(abi_msp430_interrupt)]
        });
    }

    if target != Target::None && target != Target::CortexM && target != Target::RISCV {
        output.tokens.push(quote! {
            #![cfg_attr(feature = "rt", feature(global_asm))]
            #![cfg_attr(feature = "rt", feature(use_extern_macros))]
            #![cfg_attr(feature = "rt", feature(used))]
        });
    }

    output.tokens.push(quote! {
        #![doc = #doc]
        #![deny(missing_docs)]
        #![deny(warnings)]
        #![allow(non_camel_case_types)]
        #![no_std]
    });

    match target {
        Target::CortexM => {
            output.tokens.push(quote! {
                extern crate cortex_m;
                #[cfg(feature = "rt")]
                extern crate cortex_m_rt;
            });
        }
        Target::Msp430 => {
            output.tokens.push(quote! {
                extern crate msp430;
                #[cfg(feature = "rt")]
                extern crate msp430_rt;
                #[cfg(feature = "rt")]
                pub use msp430_rt::default_handler;
            });
        }
        Target::RISCV => {
            output.tokens.push(quote! {
                extern crate riscv;
                #[cfg(feature = "rt")]
                extern crate riscv_rt;
            });
        }
        Target::None => {}
    }

    // If conditionals are used, and NO peripherals are selected,
    // certain imports may be unused
    let maybe_unused = if conditional {
        Some(quote!(#[allow(unused_imports)]))
    } else {
        None
    };
    output.tokens.push(quote! {
        extern crate bare_metal;
        extern crate vcell;

        #maybe_unused
        use core::ops::Deref;
        #maybe_unused
        use core::marker::PhantomData;
    });

    // Retaining the previous assumption
    let mut fpu_present = true;

    if let Some(cpu) = d.cpu.as_ref() {
        let bits = util::unsuffixed(u64::from(cpu.nvic_priority_bits));

        output.tokens.push(quote! {
            ///Number available in the NVIC for configuring priority
            pub const NVIC_PRIO_BITS: u8 = #bits;
        });

        fpu_present = cpu.fpu_present;
    }

    output
        .tokens
        .extend(interrupt::render(target, &d.peripherals, device_x)?);

    let core_peripherals: &[_] = if fpu_present {
        &[
            "CBP", "CPUID", "DCB", "DWT", "FPB", "FPU", "ITM", "MPU", "NVIC", "SCB", "SYST",
            "TPIU",
        ]
    } else {
        &[
            "CBP", "CPUID", "DCB", "DWT", "FPB", "ITM", "MPU", "NVIC", "SCB", "SYST", "TPIU",
        ]
    };

    let mut fields = vec![];
    let mut exprs = vec![];
    if target == Target::CortexM {
        output.tokens.push(quote! {
            pub use cortex_m::peripheral::Peripherals as CorePeripherals;
            #[cfg(feature = "rt")]
            pub use cortex_m_rt::interrupt;
            #[cfg(feature = "rt")]
            pub use self::Interrupt as interrupt;
        });

        if fpu_present {
            output.tokens.push(quote! {
                pub use cortex_m::peripheral::{
                    CBP, CPUID, DCB, DWT, FPB, FPU, ITM, MPU, NVIC, SCB, SYST, TPIU,
                };
            });
        } else {
            output.tokens.push(quote! {
                pub use cortex_m::peripheral::{
                    CBP, CPUID, DCB, DWT, FPB, ITM, MPU, NVIC, SCB, SYST, TPIU,
                };
            });
        }
    }

    let generic_file = std::str::from_utf8(include_bytes!("generic.rs")).unwrap();
    if generic_mod {
        writeln!(File::create("generic.rs").unwrap(), "{}", generic_file).unwrap();
    } else {
        let tokens = syn::parse_file(generic_file).unwrap().into_token_stream();

        output.tokens.push(quote! {
            #[allow(unused_imports)]
            use generic::*;
            ///Common register and bit access and modify traits
            pub mod generic {
                #tokens
            }
        });
    }

    for p in &d.peripherals {
        if target == Target::CortexM && core_peripherals.contains(&&*p.name.to_uppercase()) {
            // Core peripherals are handled above
            continue;
        }

        output
            .tokens
            .extend(peripheral::render(p, &d.peripherals, &d.defaults, nightly, conditional)?);

        if p.registers
            .as_ref()
            .map(|v| &v[..])
            .unwrap_or(&[])
            .is_empty()
            && p.derived_from.is_none()
        {
            // No register block will be generated so don't put this peripheral
            // in the `Peripherals` struct
            continue;
        }

        let upper_name = p.name.to_sanitized_upper_case();
        let snake_name = p.name.to_sanitized_snake_case();
        output.features.push(String::from(snake_name.clone()));
        let id = Ident::new(&*upper_name, Span::call_site());

        // Should we allow for conditional compilation of each peripheral?
        let gate = if conditional {
            Some(quote!(#[cfg(feature = #snake_name)]))
        } else {
            None
        };
        fields.push(quote! {
            #[doc = #upper_name]
            #gate
            pub #id: #id
        });
        exprs.push(quote!{
            #gate
            #id: #id { _marker: PhantomData }
        });
    }

    let span = Span::call_site();
    let take = match target {
        Target::CortexM => Some(Ident::new("cortex_m", span)),
        Target::Msp430 => Some(Ident::new("msp430", span)),
        Target::RISCV => Some(Ident::new("riscv", span)),
        Target::None => None,
    }
    .map(|krate| {
        quote! {
            ///Returns all the peripherals *once*
            #[inline]
            pub fn take() -> Option<Self> {
                #krate::interrupt::free(|_| {
                    if unsafe { DEVICE_PERIPHERALS } {
                        None
                    } else {
                        Some(unsafe { Peripherals::steal() })
                    }
                })
            }
        }
    });

    output.tokens.push(quote! {
        // NOTE `no_mangle` is used here to prevent linking different minor versions of the device
        // crate as that would let you `take` the device peripherals more than once (one per minor
        // version)
        #[no_mangle]
        static mut DEVICE_PERIPHERALS: bool = false;

        ///All the peripherals
        #[allow(non_snake_case)]
        pub struct Peripherals {
            #(#fields,)*
        }

        impl Peripherals {
            #take

            ///Unchecked version of `Peripherals::take`
            pub unsafe fn steal() -> Self {
                DEVICE_PERIPHERALS = true;

                Peripherals {
                    #(#exprs,)*
                }
            }
        }
    });

    Ok(output)
}
