# A sampled transient target the pass does not declare in its reads is refused

A frame-graph pass that samples a transient target (`TextureSource::TransientTargetColor` or `TransientTargetDepth`) in a draw or a compute dispatch must declare it in its `reads`, as `FramePass.reads` does in the vanilla frame graph: a transient target's lifetime is computed from declared uses only, so an undeclared one can be released before the pass runs. The engine now refuses such a binding when the draw or dispatch is submitted, with `PipelineDrawError::InvalidBinding` or `ComputeDispatchError::InvalidBinding`, records nothing and logs the pass and the transient once. Before, the binding was accepted and the command was silently dropped at execution. The error types and hook signatures are unchanged.

A pass that sampled a transient target it only wrote or never declared adds `.reads(FrameTarget::Transient(id))` to its descriptor.
