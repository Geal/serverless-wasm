use cretonne_wasm::{
  ModuleEnvironment, GlobalIndex, MemoryIndex, TableIndex,
  FunctionIndex, Table, Memory, Global, SignatureIndex,
  FuncTranslator, FuncEnvironment, GlobalValue
};
use cretonne::prelude::{settings::{self, Flags}, types::*, InstBuilder, Signature};
use cretonne::codegen::{
  ir::{self, ExternalName, Function},
  cursor::FuncCursor
};

pub struct Exportable<T> {
  /// A wasm entity.
  pub entity: T,

  /// Names under which the entity is exported.
  pub export_names: Vec<String>,
}

impl<T> Exportable<T> {
  pub fn new(entity: T) -> Self {
    Self {
      entity,
      export_names: Vec::new(),
    }
  }
}

pub struct ModuleInfo {
  pub flags: Flags,
  pub signatures: Vec<Signature>,
  pub imported_funcs: Vec<(String, String)>,
  pub functions: Vec<Exportable<SignatureIndex>>,
  pub function_bodies: Vec<Function>,
  pub memories: Vec<Exportable<Memory>>,
  pub tables: Vec<Exportable<Table>>,
  pub globals: Vec<Exportable<Global>>,
  pub start_func: Option<FunctionIndex>,
}

impl ModuleInfo {
  pub fn new() -> ModuleInfo {
    ModuleInfo {
      flags: settings::Flags::new(settings::builder()),
      signatures: Vec::new(),
      imported_funcs: Vec::new(),
      functions: Vec::new(),
      function_bodies: Vec::new(),
      memories: Vec::new(),
      tables: Vec::new(),
      globals: Vec::new(),
      start_func: None,
    }
  }
}

pub struct Env {
  pub info: ModuleInfo,
  trans: FuncTranslator,
}

impl Env {
  pub fn new() -> Env {
    Env {
      info: ModuleInfo::new(),
      trans: FuncTranslator::new(),
    }
  }
}

fn get_func_name(func_index: FunctionIndex) -> ir::ExternalName {
  ExternalName::user(0, func_index as u32)
}

impl<'data> ModuleEnvironment<'data> for Env {
  fn flags(&self) -> &Flags {
    &self.info.flags
  }

  fn get_func_name(&self, func_index: FunctionIndex) -> ExternalName {
    get_func_name(func_index)
  }

  fn declare_signature(&mut self, sig: &Signature) {
    self.info.signatures.push(sig.clone());
  }

  fn get_signature(&self, sig_index: SignatureIndex) -> &Signature {
    &self.info.signatures[sig_index]
  }

  fn declare_func_import(
      &mut self,
      sig_index: SignatureIndex,
      module: &'data str,
      field: &'data str
  ) {
    assert_eq!(
            self.info.functions.len(),
            self.info.imported_funcs.len(),
            "Imported functions must be declared first"
        );
    self.info.functions.push(Exportable::new(sig_index));
    self.info.imported_funcs.push((
      String::from(module),
      String::from(field),
    ));
    println!("declared function import {}:{}", module, field);
  }

  fn get_num_func_imports(&self) -> usize {
    self.info.imported_funcs.len()
  }

  fn declare_func_type(&mut self, sig_index: SignatureIndex) {
    self.info.functions.push(Exportable::new(sig_index));
  }

  fn get_func_type(&self, func_index: FunctionIndex) -> SignatureIndex {
    self.info.functions[func_index].entity
  }

  fn declare_global(&mut self, global: Global) {
    self.info.globals.push(Exportable::new(global));
  }

  fn get_global(&self, global_index: GlobalIndex) -> &Global {
    &self.info.globals[global_index].entity
  }

  fn declare_table(&mut self, table: Table) {
    self.info.tables.push(Exportable::new(table));
  }

  fn declare_table_elements(
      &mut self,
      table_index: TableIndex,
      base: Option<GlobalIndex>,
      offset: usize,
      elements: Vec<FunctionIndex>
  ) {
    //println!("declaring table elements at table n°{} base {:?} offset {}:{:?}", table_index, base, offset, elements);
  }

  fn declare_memory(&mut self, memory: Memory) {
    println!("declaring new memory zone, min: {}, max: {:?}, shared: {}", memory.pages_count, memory.maximum,
      memory.shared);
    self.info.memories.push(Exportable::new(memory));
  }

  fn declare_data_initialization(
      &mut self,
      memory_index: MemoryIndex,
      base: Option<GlobalIndex>,
      offset: usize, 
      data: &'data [u8]
  ) {
    println!("declaring data init for memory n°{}, base {:?}, offset {}, data: {:?}",
      memory_index, base, offset, data.len());
 }

  fn declare_func_export(
      &mut self,
      func_index: FunctionIndex,
      name: &'data str
  ) {
    println!("exporting function n°{} at '{}'", func_index, name);
    self.info.functions[func_index].export_names.push(
      String::from(name)
      )
  }

  fn declare_table_export(
      &mut self,
      table_index: TableIndex,
      name: &'data str
  ) { unimplemented!() }
  fn declare_memory_export(
      &mut self,
      memory_index: MemoryIndex,
      name: &'data str
  ) { unimplemented!() }
  fn declare_global_export(
      &mut self,
      global_index: GlobalIndex,
      name: &'data str
  ) { unimplemented!() }

  fn declare_start_func(&mut self, index: FunctionIndex) {
    debug_assert!(self.info.start_func.is_none());
    self.info.start_func = Some(index);
  }

  fn define_function_body(
      &mut self,
      body_bytes: &'data [u8]
  ) -> Result<(), String> {
    let func = {
      let mut func_environ = FuncEnv::new(&self.info);
      let function_index = self.get_num_func_imports() + self.info.function_bodies.len();
      let name = get_func_name(function_index);
      let sig = func_environ.vmctx_sig(self.get_func_type(function_index));
      let mut func = Function::with_name_signature(name, sig);
      self.trans
        .translate(body_bytes, &mut func, &mut func_environ)
        .map_err(|e| format!("{}", e))?;
      func
    };

    self.info.function_bodies.push(func);
    Ok(())
  }
}

pub struct FuncEnv<'env> {
    pub mod_info: &'env ModuleInfo,
}

impl<'env> FuncEnv<'env> {
    pub fn new(mod_info: &'env ModuleInfo) -> Self {
        Self { mod_info }
    }

    // Create a signature for `sigidx` amended with a `vmctx` argument after the standard wasm
    // arguments.
    fn vmctx_sig(&self, sigidx: SignatureIndex) -> ir::Signature {
        let mut sig = self.mod_info.signatures[sigidx].clone();
        sig.params.push(ir::AbiParam::special(
            self.native_pointer(),
            ir::ArgumentPurpose::VMContext,
        ));
        sig
    }
}

impl<'env> FuncEnvironment for FuncEnv<'env> {
    fn flags(&self) -> &settings::Flags {
        &self.mod_info.flags
    }

    fn make_global(&mut self, func: &mut ir::Function, index: GlobalIndex) -> GlobalValue {
        // Just create a dummy `vmctx` global.
        let offset = ((index * 8) as i32 + 8).into();
        let gv = func.create_global_var(ir::GlobalVarData::VMContext { offset });
        GlobalValue::Memory {
            gv,
            ty: self.mod_info.globals[index].entity.ty,
        }
    }

    fn make_heap(&mut self, func: &mut ir::Function, _index: MemoryIndex) -> ir::Heap {
        // Create a static heap whose base address is stored at `vmctx+0`.
        let gv = func.create_global_var(ir::GlobalVarData::VMContext { offset: 0.into() });

        func.create_heap(ir::HeapData {
            base: ir::HeapBase::GlobalVar(gv),
            min_size: 0.into(),
            guard_size: 0x8000_0000.into(),
            style: ir::HeapStyle::Static { bound: 0x1_0000_0000.into() },
        })
    }

    fn make_indirect_sig(&mut self, func: &mut ir::Function, index: SignatureIndex) -> ir::SigRef {
        // A real implementation would probably change the calling convention and add `vmctx` and
        // signature index arguments.
        func.import_signature(self.vmctx_sig(index))
    }

    fn make_direct_func(&mut self, func: &mut ir::Function, index: FunctionIndex) -> ir::FuncRef {
        let sigidx = self.mod_info.functions[index].entity;
        // A real implementation would probably add a `vmctx` argument.
        // And maybe attempt some signature de-duplication.
        let signature = func.import_signature(self.vmctx_sig(sigidx));
        let name = get_func_name(index);
        func.import_function(ir::ExtFuncData {
            name,
            signature,
            colocated: false,
        })
    }

    fn translate_call_indirect(
        &mut self,
        mut pos: FuncCursor,
        _table_index: TableIndex,
        _sig_index: SignatureIndex,
        sig_ref: ir::SigRef,
        callee: ir::Value,
        call_args: &[ir::Value],
    ) -> ir::Inst {
        // Pass the current function's vmctx parameter on to the callee.
        let vmctx = pos.func
            .special_param(ir::ArgumentPurpose::VMContext)
            .expect("Missing vmctx parameter");

        // The `callee` value is an index into a table of function pointers.
        // Apparently, that table is stored at absolute address 0 in this dummy environment.
        // TODO: Generate bounds checking code.
        let ptr = self.native_pointer();
        let callee_offset = if ptr == I32 {
            pos.ins().imul_imm(callee, 4)
        } else {
            let ext = pos.ins().uextend(I64, callee);
            pos.ins().imul_imm(ext, 4)
        };
        let mut mflags = ir::MemFlags::new();
        mflags.set_notrap();
        mflags.set_aligned();
        let func_ptr = pos.ins().load(ptr, mflags, callee_offset, 0);

        // Build a value list for the indirect call instruction containing the callee, call_args,
        // and the vmctx parameter.
        let mut args = ir::ValueList::default();
        args.push(func_ptr, &mut pos.func.dfg.value_lists);
        args.extend(call_args.iter().cloned(), &mut pos.func.dfg.value_lists);
        args.push(vmctx, &mut pos.func.dfg.value_lists);

        pos.ins()
            .CallIndirect(ir::Opcode::CallIndirect, VOID, sig_ref, args)
            .0
    }

    fn translate_call(
        &mut self,
        mut pos: FuncCursor,
        _callee_index: FunctionIndex,
        callee: ir::FuncRef,
        call_args: &[ir::Value],
    ) -> ir::Inst {
        // Pass the current function's vmctx parameter on to the callee.
        let vmctx = pos.func
            .special_param(ir::ArgumentPurpose::VMContext)
            .expect("Missing vmctx parameter");

        // Build a value list for the call instruction containing the call_args and the vmctx
        // parameter.
        let mut args = ir::ValueList::default();
        args.extend(call_args.iter().cloned(), &mut pos.func.dfg.value_lists);
        args.push(vmctx, &mut pos.func.dfg.value_lists);

        pos.ins().Call(ir::Opcode::Call, VOID, callee, args).0
    }

    fn translate_grow_memory(
        &mut self,
        mut pos: FuncCursor,
        _index: MemoryIndex,
        _heap: ir::Heap,
        _val: ir::Value,
    ) -> ir::Value {
        pos.ins().iconst(I32, -1)
    }

    fn translate_current_memory(
        &mut self,
        mut pos: FuncCursor,
        _index: MemoryIndex,
        _heap: ir::Heap,
    ) -> ir::Value {
        pos.ins().iconst(I32, -1)
    }
}
