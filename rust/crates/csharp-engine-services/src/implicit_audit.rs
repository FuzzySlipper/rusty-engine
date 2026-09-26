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
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
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
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, &mut (), operation_error, |b| {
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

pub(super) struct AnalysisLease {
    diagnostics: Vec<NativeImplicitAnalysisDiagnostic>,
    path: Vec<NativeVec3>,
}
fn pieces(
    b: &mut RuntimeImplicitBridge,
    audit: NativeImplicitAuditHandle,
) -> Result<Vec<Arc<Piece>>> {
    Ok(b.stage()?
        .audits
        .get(&audit.value)
        .ok_or_else(|| error("unknown implicit audit"))?
        .pieces
        .values()
        .cloned()
        .collect())
}
fn retain_analysis(
    b: &mut RuntimeImplicitBridge,
    report: audit::AnalysisReport,
) -> Result<NativeImplicitAnalysisReportLease> {
    use audit::AnalysisClassification as C;
    use NativeImplicitAnalysisClassification as N;
    let lease = AnalysisLease {
        diagnostics: report
            .diagnostics
            .into_iter()
            .map(|d| NativeImplicitAnalysisDiagnostic {
                piece_a: d.piece_a,
                piece_b: d.piece_b,
                classification: match d.classification {
                    C::OpenBoundary => N::OpenBoundary,
                    C::IntentionalBoundary => N::IntentionalBoundary,
                    C::NonManifold => N::NonManifold,
                    C::DegenerateTriangle => N::DegenerateTriangle,
                    C::JoinGap => N::JoinGap,
                    C::MissingJoinSurface => N::MissingJoinSurface,
                    C::EnclosureLeak => N::EnclosureLeak,
                    C::IntentionalOpening => N::IntentionalOpening,
                    C::IncompleteCoverage => N::IncompleteCoverage,
                },
                minimum: nv(d.bounds.min),
                maximum: nv(d.bounds.max),
                approximate_width: d.approximate_width,
                approximate_length: d.approximate_length,
                approximate_area: d.approximate_area,
                resolution: d.resolution,
            })
            .collect(),
        path: report.path.into_iter().map(nv).collect(),
    };
    let value = b.next_audit_report;
    b.next_audit_report = value
        .checked_add(1)
        .ok_or_else(|| error("analysis report identity overflow"))?;
    let native = NativeImplicitAnalysisReportLease {
        handle: NativeImplicitAnalysisReportLeaseHandle { value },
        sampled: report.sampled,
        complete: u8::from(report.complete),
        resolution: report.resolution,
        diagnostics: lease.diagnostics.as_ptr(),
        diagnostics_len: lease.diagnostics.len(),
        path: lease.path.as_ptr(),
        path_len: lease.path.len(),
    };
    b.analysis_reports.insert(value, lease);
    Ok(native)
}
unsafe fn copied_slice<T: Copy>(ptr: *const T, len: usize) -> Result<Vec<T>> {
    if len == 0 {
        return Ok(vec![]);
    }
    if ptr.is_null() || len > isize::MAX as usize / std::mem::size_of::<T>() {
        return Err(error("invalid analysis slice"));
    }
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec())
}
pub(super) unsafe extern "C" fn read_mesh_integrity(
    context: *mut c_void,
    request: *const NativeImplicitIntegrityRequest,
    result: *mut NativeImplicitAnalysisReportLease,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    call_operation(context, result, receipt, b"ReadMeshIntegrity", |b| {
        let openings = unsafe { copied_slice(request.openings, request.openings_len) }?
            .into_iter()
            .map(|r| audit::OpenRegion {
                piece_id: r.piece_id,
                bounds: Bounds {
                    min: v(r.minimum),
                    max: v(r.maximum),
                },
            })
            .collect::<Vec<_>>();
        let report =
            audit::integrity::inspect(&pieces(b, request.audit)?, &openings).map_err(kernel)?;
        retain_analysis(b, report)
    })
}
pub(super) unsafe extern "C" fn read_expected_join(
    context: *mut c_void,
    request: NativeImplicitJoinRequest,
    result: *mut NativeImplicitAnalysisReportLease,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    call_operation(context, result, receipt, b"ReadExpectedJoin", |b| {
        let report = audit::continuity::expected_join(
            &pieces(b, request.audit)?,
            audit::continuity::Join {
                piece_a: request.piece_a,
                piece_b: request.piece_b,
                center: v(request.center),
                half_u: v(request.half_u),
                half_v: v(request.half_v),
                search_distance: request.search_distance,
                tolerance_cells: request.tolerance_cells,
                sample_spacing: request.sample_spacing,
                max_samples: request.max_samples,
            },
        )
        .map_err(kernel)?;
        retain_analysis(b, report)
    })
}
pub(super) unsafe extern "C" fn read_enclosure(
    context: *mut c_void,
    request: *const NativeImplicitEnclosureRequest,
    result: *mut NativeImplicitAnalysisReportLease,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    call_operation(context, result, receipt, b"ReadEnclosure", |b| {
        let openings = unsafe { copied_slice(request.openings, request.openings_len) }?
            .into_iter()
            .map(|r| Bounds {
                min: v(r.minimum),
                max: v(r.maximum),
            })
            .collect();
        let report = audit::continuity::enclosure(
            &pieces(b, request.audit)?,
            &audit::continuity::Enclosure {
                bounds: Bounds {
                    min: v(request.minimum),
                    max: v(request.maximum),
                },
                interior: v(request.interior),
                openings,
                sample_spacing: request.sample_spacing,
                max_samples: request.max_samples,
            },
        )
        .map_err(kernel)?;
        retain_analysis(b, report)
    })
}
pub(super) unsafe extern "C" fn destroy_analysis_report_lease(
    context: *mut c_void,
    handle: NativeImplicitAnalysisReportLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeImplicitBridge>() };
    i32::from(handle.value != 0 && bridge.analysis_reports.remove(&handle.value).is_some())
}
