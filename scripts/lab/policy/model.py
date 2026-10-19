"""Small causal transformer over structured scene and tool tokens (no pretrained LLM)."""
import mlx.core as mx
import mlx.nn as nn
from .codec import VOCAB


class ToolPolicy(nn.Module):
    def __init__(self, dims=160, layers=4, heads=4, context=1024):
        super().__init__()
        self.embedding=nn.Embedding(VOCAB,dims)
        self.position=nn.Embedding(context,dims)
        self.blocks=nn.TransformerEncoder(layers,dims,heads,mlp_dims=dims*4,dropout=0.0,
                                          norm_first=True,checkpoint=True)

    def __call__(self,tokens):
        length=tokens.shape[1]
        x=self.embedding(tokens)+self.position(mx.arange(length))
        mask=nn.MultiHeadAttention.create_additive_causal_mask(length).astype(x.dtype)
        x=self.blocks(x,mask)
        return self.embedding.as_linear(x)


def action_loss(model,row):
    tokens=mx.array(row['tokens'],dtype=mx.int32)[None,:]
    logits=model(tokens[:,:-1])
    start=row['action_start']-1
    return nn.losses.cross_entropy(logits[:,start:,:],tokens[:,start+1:],reduction='mean')
