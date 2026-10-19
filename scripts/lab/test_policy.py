"""The policy format and target boundary work without an ML runtime."""
import unittest
from policy.codec import encode, decode, allowed, TOOL, TOOLS
from policy.context import encode_context
from splits import split_for

LEGAL={'width':12,'height':10,'planes':2,'palette':[{'id':10,'kind':0,'size':[1,1]},{'id':20,'kind':10,'size':[2,1]}],
       'overlays':[8],'underlays':[3],'remaining':100}
class PolicyCodecTests(unittest.TestCase):
    def test_tools_round_trip_including_signed_heights_and_optional_channels(self):
        obj={'id':10,'kind':0,'x':2,'y':3,'plane':0,'orientation':2}
        actions=[{'tool':'place','object':obj},{'tool':'remove','object':obj},
                 {'tool':'wall_line','object':obj,'length':4,'vertical':True},
                 {'tool':'room_rect','object':obj,'width':5,'height':4,'corner_id':None},
                 {'tool':'height_stroke','x':0,'y':1,'width':2,'height':3,'plane':0,'value':-12345},
                 {'tool':'paint_rect','x':0,'y':1,'width':2,'height':3,'plane':0,'overlay':8,'underlay':3,'shape':3,'rotation':2,'flags':None},
                 {'tool':'move','object':obj,'dx':-2,'dy':3}, {'tool':'turn','object':obj,'turns':3}]
        for action in actions:
            self.assertEqual(action,decode(encode(action,LEGAL),LEGAL))
        self.assertIsNone(decode(encode(None,LEGAL),LEGAL))
    def test_target_data_is_refused_and_missing_assets_are_not_guessed(self):
        obs={'current':{'tiles':[],'objects':[],'heights':[]},'legal':LEGAL}
        self.assertTrue(encode_context(obs,{}))
        with self.assertRaises(ValueError):
            encode_context({**obs,'target':{}},{})
        with self.assertRaises(StopIteration):
            encode({'tool':'place','object':{'id':99,'kind':0,'x':1,'y':1,'plane':0,'orientation':0}},LEGAL)
    def test_grammar_keeps_bounds_and_remove_occurrences_in_the_visible_scene(self):
        tool=TOOL+TOOLS.index('place')
        self.assertEqual(allowed([tool,257],LEGAL,{'objects':[]}),list(range(12)))
        remove=TOOL+TOOLS.index('remove')
        current={'objects':[{'id':10,'kind':0,'x':7,'y':4,'plane':0,'orientation':1}]}
        self.assertEqual(allowed([remove,257],LEGAL,current),[7])
    def test_geographic_split_matches_rust_and_never_splits_derived_variants(self):
        self.assertEqual(split_for('geography:0:0'),'test')
        self.assertEqual(split_for('geography:1:0'),'validation')
        self.assertEqual(split_for('geography:-1:0'),'train')

if __name__=='__main__': unittest.main()
