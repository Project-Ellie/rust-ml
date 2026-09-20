# rust-ml Reference Catalogue

## Abstract

This document is the canonical literature reference for the rust-ml Gomoku/AlphaZero project. It lists every external paper, thesis, rule source, verified dataset, open-source engine, chapters, tutorials, and code comments should cite when they use a pointy term such as PUCT, UCB1, threat-space search, or root Dirichlet noise. Each entry gives a full citation, a one-line reason the project cites it, a stable URL, and—where applicable—the "first noteworthy use of" tag that pins the term to its original source.

## Glossary

- **First noteworthy use** — the earliest citable source that introduced or made standard a term or mechanism in the form the project uses it. It is not necessarily the absolute first mention in print; it is the source future writing should point to.

- **MCTS** — Monte Carlo Tree Search: the four-phase select/expand/evaluate/backup algorithm.

- **UCT/PUCT** — selection rules that use upper-confidence bounds; PUCT adds a prior.

- **TSS** — threat-space search, the forcing-move proof method for Gomoku.

- **VCF/VCT** — Victory by Continuous Four / Victory by Continuous Threat (standard Gomoku/Renju terminology; no single citable origin was verified for this catalogue).

- **Local** — a document inside this repository's `docs/` tree.

## MCTS foundations and taxonomy

**Rémi Coulom (2007).** *Efficient Selectivity and Backup Operators in Monte-Carlo Tree Search*. Computers and Games (CG 2006), Lecture Notes in Computer Science 4630.
- Why this project cites it: First use of the term "Monte-Carlo Tree Search" (MCTS) and the four-phase select/expand/evaluate/backup loop.
- Link: https://doi.org/10.1007/978-3-540-75538-8_7
- First noteworthy use of: MCTS, Monte Carlo Tree Search, select, expand, evaluate, backup, four-phase MCTS

**Peter Auer, Nicolò Cesa-Bianchi, Paul Fischer (2002).** *Finite-time Analysis of the Multiarmed Bandit Problem*. Machine Learning 47(2–3), 235–256.
- Why this project cites it: UCB1: the finite-time upper-confidence-bound bandit algorithm that underlies UCT.
- Link: https://doi.org/10.1023/a:1013689704352
- First noteworthy use of: UCB1, upper confidence bound, multi-armed bandit

**Levente Kocsis, Csaba Szepesvári (2006).** *Bandit Based Monte-Carlo Planning*. European Conference on Machine Learning (ECML 2006), Lecture Notes in Computer Science 4212.
- Why this project cites it: UCT: applies the UCB1 bandit formula to tree search.
- Link: https://doi.org/10.1007/11871842_29
- First noteworthy use of: UCT, Upper Confidence bounds applied to Trees

**Christopher D. Rosin (2011).** *Multi-armed bandits with episode context*. Annals of Mathematics and Artificial Intelligence 61(3), 203–230.
- Why this project cites it: PUCT: polynomial/predictor UCT that incorporates a prior into the exploration bonus; the formula later used by AlphaGo Zero.
- Link: https://doi.org/10.1007/s10472-011-9258-6
- First noteworthy use of: PUCT, predictor UCT, polynomial UCT, P-UCT

**Sylvain Gelly, David Silver (2007).** *Combining online and offline knowledge in UCT*. Proceedings of the 24th International Conference on Machine Learning (ICML 2007).
- **link unverified** — the DOI/publisher page returned a non-200 status (or a WAF challenge) during curl checking; the DOI itself is recorded correctly.
- Why this project cites it: RAVE/AMAF: rapid action value estimation using all moves as first.
- Link: https://doi.org/10.1145/1273496.1273531
- First noteworthy use of: RAVE, AMAF, rapid action value estimation, all moves as first

**Sylvain Gelly, David Silver (2011).** *Monte-Carlo tree search and rapid action value estimation in computer Go*. Artificial Intelligence 175(11), 1856–1875.
- Why this project cites it: Definitive journal treatment of RAVE in computer Go; part of the MoGo/CrazyStone lineage.
- Link: https://doi.org/10.1016/j.artint.2011.03.007
- First noteworthy use of: RAVE

**Guillaume Chaslot, Mark H. M. Winands, H. Jaap van den Herik (2008).** *Parallel Monte-Carlo Tree Search*. Computers and Games (CG 2008), Lecture Notes in Computer Science 5131.
- Why this project cites it: Virtual loss and the parallel-MCTS taxonomy (root/leaf/tree parallelization).
- Link: https://doi.org/10.1007/978-3-540-87608-3_6
- First noteworthy use of: virtual loss, parallel MCTS, tree parallelization, leaf parallelization, root parallelization

**Cameron Browne, Edward J. Powley, Daniel Whitehouse, Simon M. Lucas, Peter I. Cowling, Philipp Rohlfshagen, Stephen Tavener, Diego Pérez, Spyridon Samothrakis, Simon Colton (2012).** *A Survey of Monte Carlo Tree Search Methods*. IEEE Transactions on Computational Intelligence and AI in Games 4(1), 1–43.
- **link unverified** — the DOI/publisher page returned a non-200 status (or a WAF challenge) during curl checking; the DOI itself is recorded correctly.
- Why this project cites it: Standard survey: MCTS phases, selection enhancements, rollouts, parallelization.
- Link: https://doi.org/10.1109/tciaig.2012.2186810

**Richard Segal (2011).** *On the Scalability of Parallel UCT*. Computers and Games (CG 2010), Lecture Notes in Computer Science 6515.
- Why this project cites it: Analysis of virtual loss and lock-free parallel UCT scaling.
- Link: https://doi.org/10.1007/978-3-642-17928-0_4
- First noteworthy use of: virtual loss

**Markus Enzenberger, Martin Müller, Broderick Arneson, Richard Segal (2010).** *Fuego—An Open-Source Framework for Board Games and Go Engine Based on Monte Carlo Tree Search*. IEEE Transactions on Computational Intelligence and AI in Games 2(4), 259–269.
- **link unverified** — the DOI/publisher page returned a non-200 status (or a WAF challenge) during curl checking; the DOI itself is recorded correctly.
- Why this project cites it: Open-source MCTS Go engine; engineering context for tree parallelization and the Fuego framework.
- Link: https://doi.org/10.1109/tciaig.2010.2083662
- First noteworthy use of: Fuego

**Mark H. M. Winands, Yngvi Björnsson, Jahn-Takeshi Saito (2008).** *Monte-Carlo Tree Search Solver*. Computers and Games (CG 2008), Lecture Notes in Computer Science 5131.
- Why this project cites it: MCTS-Solver: propagating proven wins/losses in MCTS, the exact-terminal-value technique we use.
- Link: https://doi.org/10.1007/978-3-540-87608-3_3
- First noteworthy use of: MCTS-Solver, MCTS solver, proven win propagation

**Yizao Wang, Sylvain Gelly (2007).** *Modifications of UCT and Sequence-Like Simulations for Monte-Carlo Go*. IEEE Symposium on Computational Intelligence and Games (CIG 2007).
- **link unverified** — the DOI/publisher page returned a non-200 status (or a WAF challenge) during curl checking; the DOI itself is recorded correctly.
- Why this project cites it: Adds pattern-based priors to UCT selection in Go; ancestor of the neural-network prior used in AlphaGo.
- Link: https://doi.org/10.1109/cig.2007.368095
- First noteworthy use of: UCT with patterns, pattern prior

**Guillaume Chaslot, Mark H. M. Winands, Jaap W. H. M. Uiterwijk (2008).** *Progressive Strategies for Monte-Carlo Tree Search*. New Mathematics and Natural Computation 4(3), 343–357.
- **link unverified** — the DOI/publisher page returned a non-200 status (or a WAF challenge) during curl checking; the DOI itself is recorded correctly.
- Why this project cites it: Progressive bias and progressive widening: classical MCTS heuristics catalogued in our exclusions.
- Link: https://doi.org/10.1142/s1793005708001094
- First noteworthy use of: progressive bias, progressive widening

## AlphaGo line

**David Silver, Aja Huang, Chris J. Maddison, Arthur Guez, Laurent Sifre, George van den Driessche, Julian Schrittwieser, Ioannis Antonoglou, Veda Panneershelvam, Marc Lanctot, Sander Dieleman, Dominik Grewe, John Nham, Nal Kalchbrenner, Ilya Sutskever, Timothy Lillicrap, Madeleine Leach, Koray Kavukcuoglu, Thore Graepel, Demis Hassabis (2016).** *Mastering the game of Go with deep neural networks and tree search*. Nature 529, 484–489.
- Why this project cites it: AlphaGo: neural network priors + MCTS + rollout policy, the first superhuman Go program.
- Link: https://doi.org/10.1038/nature16961
- First noteworthy use of: AlphaGo, policy network, value network, rollout policy

**David Silver, Julian Schrittwieser, Karen Simonyan, Ioannis Antonoglou, Aja Huang, Arthur Guez, Thomas Hubert, Lucas Baker, Matthew Lai, Adrian Bolton, Yutian Chen, Timothy Lillicrap, Fan Hui, Laurent Sifre, George van den Driessche, Thore Graepel, Demis Hassabis (2017).** *Mastering the game of Go without human knowledge*. Nature 550, 354–359.
- Why this project cites it: AlphaGo Zero: self-play only, no human data; root Dirichlet noise, temperature sampling, resignation thresholding, tree reuse.
- Link: https://doi.org/10.1038/nature24270
- First noteworthy use of: AlphaGo Zero, root Dirichlet noise, Dirichlet noise, temperature sampling, resignation threshold, tree reuse

**David Silver, Thomas Hubert, Julian Schrittwieser, Ioannis Antonoglou, Matthew Lai, Arthur Guez, Marc Lanctot, Laurent Sifre, Dharshan Kumaran, Thore Graepel, Timothy Lillicrap, Karen Simonyan, Demis Hassabis (2018).** *Mastering Chess and Shogi by Self-Play with a General Reinforcement Learning Algorithm*. Science 362(6419), 1140–1144.
- Why this project cites it: AlphaZero: generalizes AGZ to chess and shogi; states the Dirichlet alpha heuristic (alpha ≈ 10 / typical legal moves).
- Link: https://arxiv.org/abs/1712.01815
- First noteworthy use of: AlphaZero

**Julian Schrittwieser, Ioannis Antonoglou, Thomas Hubert, Karen Simonyan, Laurent Sifre, Simon Schmitt, Arthur Guez, Edward Lockhart, Demis Hassabis, Thore Graepel, Timothy Lillicrap, David Silver (2020).** *Mastering Atari, Go, Chess and Shogi by Planning with a Learned Model*. Nature 588, 604–609.
- Why this project cites it: MuZero: model-based planning without knowing the rules; included as context, not implemented.
- Link: https://arxiv.org/abs/1911.08265
- First noteworthy use of: MuZero

**David Silver et al. (2017).** *AlphaGo Zero — Supplementary Information*. Nature 550 (supplementary methods).
- Why this project cites it: Supplementary methods for AlphaGo Zero: network architecture, hyperparameters, self-play pipeline details.
- Link: https://doi.org/10.1038/nature24270

## Industrial reimplementations and engineering

**Yuandong Tian, Jerry Ma, Qucheng Gong, Shubho Sengupta, Zhuoyuan Chen, James Pinkerton, C. Lawrence Zitnick (2019).** *ELF OpenGo: An Analysis and Open Reimplementation of AlphaZero*. Proceedings of the 36th International Conference on Machine Learning (ICML 2019).
- Why this project cites it: ELF OpenGo: open reimplementation; publishes c_puct = 1.5 and batching/engineering practice.
- Link: https://arxiv.org/abs/1902.04522
- First noteworthy use of: ELF OpenGo, c_puct

**David J. Wu (2019).** *Accelerating Self-Play Learning in Go*. arXiv:1902.10565.
- Why this project cites it: KataGo: global pooling, playout-cap randomization, auxiliary targets, forced playouts.
- Link: https://arxiv.org/abs/1902.10565
- First noteworthy use of: KataGo, global pooling, playout-cap randomization

**Tristan Cazenave, Yen-Chi Chen, Guan-Wei Chen, Shi-Yu Chen, Xian-Dong Chiu, Julien Dehos, Maria Elsa, Qucheng Gong, Hengyuan Hu, Vasil Khalidov, Cheng-Ling Li, Hsin-I Lin, Yu-Jin Lin, Xavier Martinet, Vegard Mella, Jérémy Rapin, Baptiste Rozière, Gabriel Synnaeve, Fabien Teytaud, Olivier Teytaud, Shi-Cheng Ye, Yi-Jun Ye, Shi-Jim Yen, Sergey Zagoruyko (2020).** *Polygames: Improved Zero Learning*. ICGA Journal 43(1), 1–17.
- Why this project cites it: Polygames: general board-game zero-learning framework; planar/hexagonal variants and global pooling.
- Link: https://arxiv.org/abs/2001.09832
- First noteworthy use of: Polygames

**Gian-Carlo Pascutto (GCP) and contributors (2017).** *Leela Zero*. Open-source project.
- Why this project cites it: Community AlphaZero reimplementation for Go; the main public baseline for zero-learning engines.
- Link: https://zero.sjeng.org/
- First noteworthy use of: Leela Zero

## Gomoku, threat-space search, and opening rules

**Zheng Xie, Xingyu Fu, JinYuan Yu (2018).** *AlphaGomoku: An AlphaGo-based Gomoku Artificial Intelligence using Curriculum Learning*. arXiv:1809.10595.
- Why this project cites it: Closest published AlphaZero-style Gomoku agent; curriculum learning for the short-horizon tactical nature of Gomoku.
- Link: https://arxiv.org/abs/1809.10595
- First noteworthy use of: AlphaGomoku

**Louis Victor Allis (1994).** *Searching for Solutions in Games and Artificial Intelligence*. PhD thesis, Maastricht University.
- Why this project cites it: Origin of threat-space search (TSS); proof that Go-Moku is a first-player win; proof-number search background.
- Link: https://doi.org/10.26481/dis.19940923la
- First noteworthy use of: threat-space search, TSS, Go-Moku, proof-number search, dependency-based search

**Louis Victor Allis, H. Jaap van den Herik, M. P. H. Huntjens (1996).** *Go-Moku Solved by New Search Techniques*. Computational Intelligence 12(1), 7–23.
- **link unverified** — the DOI/publisher page returned a non-200 status (or a WAF challenge) during curl checking; the DOI itself is recorded correctly.
- Why this project cites it: Go-Moku solved via threat-space search and proof-number search; companion to the 1994 thesis.
- Link: https://doi.org/10.1111/j.1467-8640.1996.tb00250.x
- First noteworthy use of: Go-Moku solved, threat-space search

**I-Chen Wu, Dei-Yen Huang, Hsiu-Chen Chang (2005).** *Connect6*. ICGA Journal 28(4), 234–242.
- **link unverified** — the DOI/publisher page returned a non-200 status (or a WAF challenge) during curl checking; the DOI itself is recorded correctly.
- Why this project cites it: Connect6: six-in-a-row connect game; extends Gomoku ideas to a modern variant with opening fairness.
- Link: https://doi.org/10.3233/icg-2005-28405
- First noteworthy use of: Connect6

**Renju International Federation (None).** *Renju Rules*. renju.net.
- Why this project cites it: Official Renju rules and opening terminology; context for Gomoku opening-rule history.
- Link: https://renju.net/rules/
- First noteworthy use of: Renju rules

**Gomocup organizers (None).** *Gomocup: the Gomoku AI Tournament — Detail Information*. gomocup.org.
- Why this project cites it: Gomocup tournament rules: freestyle, standard, renju, caro, balanced openings.
- Link: https://gomocup.org/detail-information/
- First noteworthy use of: Gomocup, freestyle gomoku, standard gomoku, balanced openings

## External data sources for training and puzzles

**Gomocup organizers (None).** *Gomocup tournament result archives (2000–2026)*. gomocup.org.
- Why this project cites it: Yearly ZIP archives of Gomocup tournament games in PSQ format, plus balanced opening files for freestyle, standard, caro and renju rulesets.
- Link: https://gomocup.org/results/

**Petr Lastovicka (plastovicka) (None).** *Piskvork openings.txt*. GitHub.
- Why this project cites it: Small, clean Swap2 opening catalogue shipped with the Piskvork tournament manager: 41 lines of comma-separated (x,y) coordinate pairs relative to the board centre.
- Link: https://raw.githubusercontent.com/plastovicka/Piskvork/master/openings.txt

**Renju International Federation (None).** *International Rules of Gomoku — Swap2*. renju.net.
- Why this project cites it: Authoritative HTML description of the Swap2 opening protocol and the international Gomoku rule set. Note: RIF Gomoku rules treat overlines as non-winning, unlike the project's freestyle ruleset.
- Link: https://www.renju.net/gomokurules/

**Karesis (None).** *Karesis / Gomoku (Five in a Row) AI Dataset*. Hugging Face Datasets.
- Why this project cites it: MIT-licensed HuggingFace dataset: 875 self-play games / 26,378 15×15 board-state/next-move positions generated with the WinePy alpha-beta engine.
- Link: https://doi.org/10.57967/hf/4816

**PoolC (None).** *PoolC / gomoku-dataset-1.8M*. Hugging Face Datasets.
- Why this project cites it: Large HuggingFace dataset of 1.88M tokenized Gomoku positions in a single Parquet file. License and encoding are undocumented; unusable without reverse-engineering the input_ids sequence.
- Link: https://huggingface.co/datasets/PoolC/gomoku-dataset-1.8M

**gugujiao953-ship-it (None).** *banbu-gomoku VCF material*. GitHub.
- Why this project cites it: 763 labelled VCF (victory by continuous four) puzzles with explicit solution lines. The only verified public set found that ships answers; ruleset must be re-verified for freestyle overlines-win.
- Link: https://raw.githubusercontent.com/gugujiao953-ship-it/banbu-gomoku/main/public/puzzles/vcf-material.json

**gugujiao953-ship-it (None).** *banbu-gomoku 开宝实战VCF*. GitHub.
- Why this project cites it: ~1050 VCF puzzle positions imported from the Kaibao Android app. No stored answers; Renju-oriented; requires an internal solver to produce labels.
- Link: https://raw.githubusercontent.com/gugujiao953-ship-it/banbu-gomoku/main/public/puzzles/kaibao/%E5%AE%9E%E6%88%98VCF_1052%E9%A2%98.json

**gugujiao953-ship-it (None).** *banbu-gomoku RenjuPortal VCF collection*. GitHub.
- Why this project cites it: 4,024 VCF puzzle positions from RenjuPortal. No stored solution lines; Renju-oriented; requires ruleset conversion before use in freestyle Gomoku.
- Link: https://raw.githubusercontent.com/gugujiao953-ship-it/banbu-gomoku/main/public/puzzles/kaibao/RenjuPortalVCF.json

**lfz084 (None).** *lfz084 / renju puzzle JSON archive*. GitHub.
- Why this project cites it: Large collection of Renju puzzle sets (黑先VCF, 白先VCF, mate-in-3, exercises). Thousands of positions but no explicit answers; strong Renju bias and forbidden-hand conventions.
- Link: https://github.com/lfz084/renju/tree/master/puzzle/json

**Tk-visionary (None).** *renju-benchmark synthetic tactical tests*. GitHub.
- Why this project cites it: GPL-3.0 set of 8 hand-authored labelled Renju positions (exact-five, overline, forbidden moves, must-block). Small correctness/edge-case test set, not a large training corpus.
- Link: https://raw.githubusercontent.com/Tk-visionary/renju-benchmark/main/data/puzzles.jsonl

**maojh15 (None).** *maojh15 / GomokuZeroAI*. Hugging Face Models.
- Why this project cites it: MIT-licensed AlphaZero-style Gomoku policy-value checkpoint for 15×15. Could serve as a sparring partner or synthetic-data generator, or as an initialization for transfer learning.
- Link: https://huggingface.co/maojh15/GomokuZeroAI

**Nagi-ovo (None).** *Nagi-ovo / alphazero-gomoku*. Hugging Face Models.
- Why this project cites it: MIT-licensed AlphaZero-style Gomoku model checkpoint trained for the same-named GitHub project. Potential sparring partner or pretrained initialization; verify board size and ruleset.
- Link: https://huggingface.co/Nagi-ovo/alphazero-gomoku

## Open-source Gomoku engines and tools

**dhbloo (None).** *Rapfi — strong open-source Gomoku/Renju engine*. GitHub.
- Why this project cites it: Modern alpha-beta + NNUE engine and one of the strongest public Gomoku programs. Supports selfplay mode that outputs training samples and an opengen mode for balanced openings. GPL-3.0 (engine); network weights are CC0-1.0.
- Link: https://github.com/dhbloo/rapfi

**Maciej Kozarzewski (None).** *AlphaGomoku — AlphaZero-style Gomoku engine*. GitHub.
- Why this project cites it: Full C++ AlphaZero implementation for Gomoku with MCTS, neural networks, selfplay and training front-ends. Supports Swap2 and other opening controllers. GPL-3.0.
- Link: https://github.com/MaciejKozarzewski/AlphaGomoku

**hzyhhzy (None).** *KataGomo — KataGo fork for Gomoku and Renju*. GitHub.
- Why this project cites it: KataGo derivative trained for Gomoku (freestyle, standard) and Renju. Strong open-source engine with pre-trained network releases and KataGo selfplay/training scripts. Custom MIT-style license.
- Link: https://github.com/hzyhhzy/KataGomo

**nkg114mc (None).** *c-gomoku-cli — head-to-head match runner for Gomocup engines*. GitHub.
- Why this project cites it: Command-line tournament manager that runs any Gomocup-protocol engines, supports fixed opening files and concurrent games, and writes SGF plus CSV/binary training samples. GPL-3.0.
- Link: https://github.com/nkg114mc/c-gomoku-cli

**dhbloo (None).** *Rapfi-gomocup — legacy Gomocup freestyle engine*. GitHub.
- Why this project cites it: Older MIT-licensed Rapfi that participated in Gomocup 2018. Speaks the Gomocup protocol and can be driven by c-gomoku-cli or Piskvork. Weaker than current Rapfi but permissively licensed.
- Link: https://github.com/dhbloo/Rapfi-gomocup

**wind23 (None).** *SlowRenju — Gomoku/Renju alpha-beta engine*. GitHub.
- Why this project cites it: Iterative-deepening alpha-beta engine that has competed in Gomocup since 2013. Supports freestyle, standard and renju on board sizes 5–20. GPL-3.0; useful mid-tier sparring partner.
- Link: https://github.com/wind23/SlowRenju

**schibir (None).** *PentaZen — alpha-beta Gomoku/Renju engine*. GitHub.
- Why this project cites it: Strong open-source alpha-beta engine (Gomocup 2021 version). Speaks Gomocup and partial Yixin protocols. GPL-3.0; can be paired with c-gomoku-cli for batch games.
- Link: https://github.com/schibir/PentaZen

**ChisBread (None).** *Chis — Gomoku AI engine*. GitHub.
- Why this project cites it: C++ Gomoku engine producing a Piskvork-compatible executable. MPL-2.0; mid-tier sparring partner via the Gomocup protocol.
- Link: https://github.com/ChisBread/Chis

**Joker2770 (None).** *Z2I — AlphaZero-style Gomoku engine*. GitHub.
- Why this project cites it: MIT-licensed MCTS+ONNX engine supporting freestyle, standard, renju and caro, with Python training scripts. Less strong than Rapfi/KataGomo but permissively licensed and includes selfplay training.
- Link: https://github.com/Joker2770/Z2I

**winterdl (None).** *Nut-engine — Gomocup protocol engine*. GitHub.
- Why this project cites it: Apache-2.0 Gomoku/Renju AI supporting the Gomocup protocol. Mid-tier sparring partner; permissive license makes it safe for integration tests.
- Link: https://github.com/winterdl/Nut-engine

## Network architecture and training

**Kaiming He, Xiangyu Zhang, Shaoqing Ren, Jian Sun (2016).** *Deep Residual Learning for Image Recognition*. Proceedings of the IEEE Conference on Computer Vision and Pattern Recognition (CVPR 2016).
- Why this project cites it: ResNet: residual blocks with skip connections, the trunk of our value/policy network.
- Link: https://arxiv.org/abs/1512.03385
- First noteworthy use of: ResNet, residual network, residual block, skip connection

**Sergey Ioffe, Christian Szegedy (2015).** *Batch Normalization: Accelerating Deep Network Training by Reducing Internal Covariate Shift*. arXiv:1502.03167.
- Why this project cites it: Batch normalization, used in the ResNet trunk.
- Link: https://arxiv.org/abs/1502.03167
- First noteworthy use of: batch normalization, batch norm, internal covariate shift

**Tom Schaul, John Quan, Ioannis Antonoglou, David Silver (2015).** *Prioritized Experience Replay*. arXiv:1511.05952.
- Why this project cites it: Prioritized replay sampling; catalogue context for the replay buffer design.
- Link: https://arxiv.org/abs/1511.05952
- First noteworthy use of: prioritized experience replay, prioritized replay

**Diederik P. Kingma, Jimmy Ba (2014).** *Adam: A Method for Stochastic Optimization*. arXiv:1412.6980.
- Why this project cites it: Adam optimizer; project training uses AdamW (Adam with decoupled weight decay).
- Link: https://arxiv.org/abs/1412.6980
- First noteworthy use of: Adam

**Ilya Loshchilov, Frank Hutter (2017).** *Decoupled Weight Decay Regularization*. arXiv:1711.05101.
- Why this project cites it: AdamW: Adam with decoupled weight decay, the optimizer planned for network training.
- Link: https://arxiv.org/abs/1711.05101
- First noteworthy use of: AdamW, decoupled weight decay

**Christian Szegedy, Vincent Vanhoucke, Sergey Ioffe, Jon Shlens, Zbigniew Wojna (2016).** *Rethinking the Inception Architecture for Computer Vision*. Proceedings of the IEEE Conference on Computer Vision and Pattern Recognition (CVPR 2016).
- Why this project cites it: Introduces label smoothing and factorized convolutions; catalogue context for training tricks.
- Link: https://arxiv.org/abs/1512.00567
- First noteworthy use of: label smoothing

## Ratings and optimization

**Rémi Coulom (2007).** *Computing "Elo Ratings" of Move Patterns in the Game of Go*. ICGA Journal 30(4), 198–208.
- **link unverified** — the DOI/publisher page returned a non-200 status (or a WAF challenge) during curl checking; the DOI itself is recorded correctly.
- Why this project cites it: Elo estimation for move patterns; background for the rating ideas later used in MCTS priors.
- Link: https://doi.org/10.3233/icg-2007-30403

**Rémi Coulom (2012).** *CLOP: Confident Local Optimization for Noisy Black-Box Parameter Tuning*. Advances in Computer Games (ACG 2011), Lecture Notes in Computer Science 7168.
- Why this project cites it: CLOP: parameter tuning with noisy evaluations; relevant if we later tune c_puct or other constants.
- Link: https://doi.org/10.1007/978-3-642-31866-5_13
- First noteworthy use of: CLOP

**Arpad E. Elo (1978).** *The Rating of Chessplayers, Past and Present*. Batsford (book).
- **unverified** — bibliographic details could not be fully confirmed against a publisher record.
- **link unverified** — the DOI/publisher page returned a non-200 status (or a WAF challenge) during curl checking; the DOI itself is recorded correctly.
- Why this project cites it: The Elo rating system used for arena strength measurement.
- Link: https://ci.nii.ac.jp/ncid/BB18462776
- First noteworthy use of: Elo rating, Elo rating system

## Local project documents

**Trace-AI (Burn maintainers) (2024).** *Burn v0.21.0 API Documentation*. Local/docs (crate docs).
- Why this project cites it: Canonical API reference for the pinned Burn version used in the project.
- Link: https://docs.rs/burn/0.21.0/burn/index.html

**rust-ml project (None).** *Chapter 12 — Gomoku Architecture*. Local/docs.
- Why this project cites it: System design, milestone plan, crate boundaries, claim-label conventions, and honesty ledger.
- Link: docs/12-gomoku-architecture.md

**rust-ml project (None).** *Chapter 13 — Engine Design*. Local/docs.
- Why this project cites it: Engine design and locked decisions: freestyle overlines, Swap2, 17×17 encoding, absolute colors.
- Link: docs/13-engine-design.md
