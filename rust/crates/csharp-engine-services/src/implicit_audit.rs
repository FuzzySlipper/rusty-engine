//! Opt-in snapshots for authored surface diagnostics. No frame-loop work.
use super::*;
use svc_implicit::audit::{self, Classification, Piece};

#[derive(Clone, Default)]
pub(super) struct AuditCollection {
    pub pieces: BTreeMap<u64, Arc<Piece>>,
}

pub(super) unsafe extern "C" fn create_audit(
    context: *mut c_void,
    result: *mut NativeImplicitAuditHandle,
) -> i32 {
    call(context, result, |b| {
        let value = b.next_audit;
        b.next_audit = value
            .checked_add(1)
            .ok_or_else(|| error("audit identity overflow"))?;
        b.stage()?.audits.insert(value, AuditCollection::default());
        Ok(NativeImplicitAuditHandle { value })
    })
}

pub(super) unsafe extern "C" fn destroy_audit(
    context: *mut c_void,
    handle: NativeImplicitAuditHandle,
) -> i32 {
    call(context, &mut (), |b| {
        b.stage()?
            .audits
            .remove(&handle.value)
            .ok_or_else(|| error("unknown implicit audit"))?;
        Ok(())
    })
}

pub(super) unsafe extern "C" fn capture_audit_piece(
    context: *mut c_void,
    request: NativeImplicitAuditPieceRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    call_operation(context, &mut (), receipt, b"CaptureAuditPiece", |b| {
        let collection = b
            .stage()?
            .audits
            .get(&request.audit.value)
            .ok_or_else(|| error("unknown implicit audit"))?;
        if collection.pieces.contains_key(&request.piece_id) {
            return Err(error("duplicate audit piece identity"));
        }
        let retained = b.retained(request.field)?;
        let node = retained.node(request.source)?;
        let field = Arc::clone(&retained.field);
        let appearance = b
            .appearance
            .ok_or_else(|| error("Graphics bridge is not bound"))?;
        // Existing Engine mesh copy mechanism; no retained native pointers or
        // renderer material dependencies belong to an audit collection.
        let (positions, triangles) =
            unsafe { &mut *appearance }.copy_inline_mesh_geometry(request.mesh)?;
        let t = request.placement;
        let piece = Piece::new_trs(
            request.piece_id,
            &field,
            node,
            positions.into_iter().map(|p| p.map(|x| x as f32)).collect(),
            triangles,
            v(t.translation),
            [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w],
            v(t.scale),
            request.sample_spacing,
        )
        .map_err(kernel)?;
        b.stage()?
            .audits
            .get_mut(&request.audit.value)
            .expect("validated audit")
            .pieces
            .insert(request.piece_id, Arc::new(piece));
        Ok(())
    })
}

pub(super) unsafe extern "C" fn read_audit(
    context: *mut c_void,
    request: NativeImplicitAuditRequest,
    result: *mut NativeImplicitAuditReportLease,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    call_operation(context, result, receipt, b"ReadAudit", |b| {
        let pieces = b
            .stage()?
            .audits
            .get(&request.audit.value)
            .ok_or_else(|| error("unknown implicit audit"))?
            .pieces
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let report = audit::audit(&pieces, request.tolerance_cells).map_err(kernel)?;
        let diagnostics = report
            .diagnostics
            .into_iter()
            .map(|d| NativeImplicitAuditDiagnostic {
                piece_a: d.piece_a,
                piece_b: d.piece_b,
                classification: match d.classification {
                    Classification::CoincidentExposed => {
                        NativeImplicitAuditClassification::CoincidentExposed
                    }
                    Classification::NearCoincidentExposed => {
                        NativeImplicitAuditClassification::NearCoincidentExposed
                    }
                    Classification::BuriedSurface => {
                        NativeImplicitAuditClassification::BuriedSurface
                    }
                },
                minimum: nv(d.bounds.min),
                maximum: nv(d.bounds.max),
                approximate_area: d.approximate_area,
            })
            .collect::<Vec<_>>();
        let value = b.next_audit_report;
        b.next_audit_report = value
            .checked_add(1)
            .ok_or_else(|| error("audit report identity overflow"))?;
        let native = NativeImplicitAuditReportLease {
            handle: NativeImplicitAuditReportLeaseHandle { value },
            candidate_pairs: report.candidate_pairs,
            triangle_pairs: report.triangle_pairs,
            diagnostics: diagnostics.as_ptr(),
            diagnostics_len: diagnostics.len(),
        };
        b.audit_reports.insert(value, diagnostics);
        Ok(native)
    })
}

pub(super) unsafe extern "C" fn destroy_audit_report_lease(
    context: *mut c_void,
    handle: NativeImplicitAuditReportLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeImplicitBridge>() };
    i32::from(handle.value != 0 && bridge.audit_reports.remove(&handle.value).is_some())
}
