mod config;
use config::BlocklessConfig;
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::{format_ident, quote};
use syn::parse_macro_input;
use wiggle_generate::names;

#[proc_macro]
pub fn linker_integration(args: TokenStream) -> proc_macro::TokenStream {
    let config = parse_macro_input!(args as BlocklessConfig);

    let doc = config.load_document();
    let mut funcs = Vec::new();
    let mut bounds = Vec::new();
    let mut module_ident = Vec::new();
    for module in doc.modules() {
        for f in module.funcs() {
            funcs.push(generate_func(&module, &f, Some(&config.target)));
        }
        bounds.push(names::trait_name(&module.name));
        module_ident.push(names::module(&module.name));
    }
    let method_name = format_ident!("{}", config.link_method.value());
    let target_path = &config.target;
    let mut ctx = bounds
        .iter()
        .zip(module_ident.iter())
        .map(|(m, b)| quote!(+#target_path::#b::#m))
        .collect::<Vec<_>>();
    ctx.push(quote!(+#target_path::types::UserErrorConversion));
    let s = quote!(
        pub fn #method_name<T, U>(
            linker: &mut Linker<T>,
            get_ctx: impl Fn(&mut T) -> &mut U + Send + Copy + Sync + 'static
        ) -> wiggle::anyhow::Result<()>
        where
            T: Send,
            U: Send #(#ctx)*
        {
            #(#funcs)*
            Ok(())
        }

    );
    s.into()
}

fn generate_func(
    module: &witx::Module,
    func: &witx::InterfaceFunc,
    target_path: Option<&syn::Path>,
) -> proc_macro2::TokenStream {
    let module_ident = names::module(&module.name);
    let module_name = module.name.as_str();
    let (params, results) = func.wasm_signature();

    let arg_names: Vec<Ident> = (0..params.len())
        .map(|i| Ident::new(&format!("arg{}", i), Span::call_site()))
        .collect::<Vec<_>>();

    let arg_decls = params
        .iter()
        .enumerate()
        .map(|(i, ty)| {
            let name = &arg_names[i];
            let wasm = names::wasm_type(*ty);
            quote! { #name: #wasm }
        })
        .collect::<Vec<_>>();

    let wrapper = format_ident!("func_wrap{}_async", params.len());
    let func_name = func.name.as_str();
    let func_ident = names::func(&func.name);
    let ret_ty = match results.len() {
        0 => quote!(()),
        1 => names::wasm_type(results[0]),
        _ => unimplemented!(),
    };
    let abi_func = quote!( #target_path::#module_ident::#func_ident );
    let linker = quote!(
        linker.#wrapper(
            #module_name,
            #func_name,
            move |mut caller: wiggle::wasmtime_crate::Caller<'_, T> #(, #arg_decls)*| {
                Box::new(async move {
                    let mem = match caller.get_export("memory") {
                        Some(wiggle::wasmtime_crate::Extern::Memory(m)) => m,
                        _ => {
                            wiggle::anyhow::bail!("missing required memory export");
                        }
                    };
                    let (mem, data) = mem.data_and_store_mut(&mut caller);
                    let mem = wiggle::wasmtime::WasmtimeGuestMemory::new(mem);
                    let ctx = get_ctx(data);
                    Ok(<#ret_ty>::from(#abi_func(ctx, &mem #(, #arg_names)*).await ?))
                })
            },
        )?;
    );
    linker.into()
}
